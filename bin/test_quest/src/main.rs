#![allow(clippy::result_large_err)]
#![allow(dead_code)]

use std::env;
use std::sync::Arc;

use clap::Parser;
use miette::Diagnostic;
use miette::Result;
use thiserror::Error;
use tokio::task::JoinHandle;

use crate::asserter::AssertResult;
use crate::asserter::Asserter;
use crate::cli::Cli;
use crate::outputter::OutPutter;
use crate::parser::TestQuest;
use crate::runner::RunnerError;
use crate::runner::RunnerResult;
use crate::runner::run_tests;
use crate::setup::StartUpError;
use crate::setup::app::AppProcess;
use crate::setup::app::OutputLine;
use crate::setup::app::OutputSource;
use crate::setup::start_db_and_app;
use crate::validator::EnvSetup;
use crate::validator::IR;
use crate::validator::ValidationError;
use crate::validator::Validator;

mod asserter;
mod cli;
mod outputter;
mod parser;
mod runner;
mod setup;
mod validator;

#[derive(Error, Debug, Diagnostic)]
pub enum TestQuestError {
    #[error("Failed to read toml file")]
    FileError(#[from] std::io::Error),

    #[error("Failed in the startup process: {0}")]
    StartUpError(StartUpError),

    #[error("Failed to parse toml file")]
    TomlParsing(#[from] toml::de::Error),

    #[error(transparent)]
    #[diagnostic(transparent)]
    ValidationError(#[from] ValidationError),

    #[error("Failed in assert step")]
    AssertError,
}
/// Loads the test configuration file and validates its contents.
///
/// This function:
/// - Parses CLI arguments to locate the configuration file.
/// - Reads and deserializes the file into a `TestQuest` structure from TOML.
/// - Runs a validation pass over the configuration to ensure correctness.
/// - Returns the parsed CLI options, validated test definitions (`IR`), the
///   total number of tests, and the environment setup information.
///
/// # Errors
/// Returns a `TestQuestError` if:
/// - The file cannot be read,
/// - The TOML fails to parse,
/// - Or the configuration validation fails.
async fn load_and_validate_config() -> Result<(Cli, IR, usize, EnvSetup), TestQuestError> {
    let cli = Cli::parse();

    let contents = std::fs::read_to_string(&cli.path).map_err(TestQuestError::FileError)?;
    let test_quest: TestQuest = toml::from_str(&contents).map_err(TestQuestError::TomlParsing)?;

    if let Some(ref env_vars) = test_quest.setup.env {
        for (key, value) in env_vars {
            // SAFETY:
            // No other threads or child processes have been spawned yet, and environment
            // variables are only being modified in the current process. Therefore, calling
            // `env::set_var` here is safe.
            unsafe {
                env::set_var(key, value);
            }
        }
    }

    let mut validator = Validator::new(&test_quest, contents.as_str(), cli.path.as_str());

    let (test_groups, setup) = validator
        .validate()
        .map_err(TestQuestError::ValidationError)?;
    let n_tests = test_groups.tests.len();

    Ok((cli, test_groups, n_tests, setup))
}

/// Spawns the concurrent test pipeline tasks: runner, asserter, and outputter.
///
/// This function sets up communication channels between the three pipeline
/// stages:
/// - **Runner:** Executes each HTTP test and sends the results to the asserter.
/// - **Asserter:** Validates test results and forwards assertion outcomes to
///   the outputter.
/// - **Outputter:** Collects and prints or writes test results to disk.
///
/// Each stage runs in its own Tokio task with unbounded flume channels for
/// communication. The function returns the join handles for all three tasks so
/// they can be awaited later.
///
/// # Concurrency
/// All three tasks run concurrently and communicate via flume channels.
async fn run_pipeline_tasks(
    test_groups: IR,
    n_tests: usize,
    pool: &sqlx::Pool<sqlx::Any>,
    path: &str,
) -> (
    JoinHandle<Result<(), RunnerError>>,
    JoinHandle<Result<(), ()>>,
    JoinHandle<()>,
) {
    let (runner_tx, asserter_rx) = flume::unbounded::<RunnerResult>();
    let (asserter_tx, outputter_rx) =
        flume::unbounded::<(String, String, String, Arc<[AssertResult]>)>();

    // Outputter Task
    let outputter_rx_printter = outputter_rx.clone();
    let outputter_path = path.to_owned();

    let outputter_handle = tokio::spawn(async move {
        OutPutter::start(outputter_rx_printter, &outputter_path, n_tests).await;
    });

    // TestRunner Task
    let pool = pool.clone();

    let runner_jh =
        tokio::spawn(async move { run_tests(test_groups, runner_tx, pool.clone()).await });

    // Asserter Task
    let asserter_outputter_tx = asserter_tx;

    let asserter_jh =
        tokio::spawn(async move { Asserter::run(asserter_rx, asserter_outputter_tx).await });

    (runner_jh, asserter_jh, outputter_handle)
}

/// Waits for all pipeline tasks to finish and then terminates the running app
/// process.
async fn cleanup_and_teardown(
    process: &AppProcess,
    runner_jh: JoinHandle<Result<(), RunnerError>>,
    asserter_jh: JoinHandle<Result<(), ()>>,
    outputter_handle: JoinHandle<()>,
) {
    let _ = futures::join!(runner_jh, asserter_jh, outputter_handle);

    let mut lock = process.process.lock().await;
    let _ = lock.kill().await;
}

/// Prints the captured stdout and stderr from the application process.
///
/// Displays each output line with its source label (`[STDOUT]` or `[STDERR]`)
/// to help distinguish interleaved output streams.
async fn print_app_output(output_lines: &Arc<tokio::sync::Mutex<Vec<OutputLine>>>) {
    let output = output_lines.lock().await;

    println!("\n--- Captured Interleaved Output ---");
    for item in output.iter() {
        match item.source {
            // TODO: Use console::style or similar for color here
            OutputSource::StdOut => println!("[STDOUT] {}", item.line),
            OutputSource::StdErr => eprintln!("[STDERR] {}", item.line),
        }
    }
    println!("------------------------------------");
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load the CLI arguments and read the test configuration file.
    // The configuration is parsed, validated, and returned together with
    // the total number of tests and environment setup details.
    let (cli, test_groups, n_tests, setup) = load_and_validate_config().await?;

    // Start the database container (e.g. Postgres, MySQL, etc.) and launch
    // the application under test. Returns a handle containing the process,
    // database connection pool, and captured output buffers.
    let app_handle = start_db_and_app(setup, cli.stream_app)
        .await
        .map_err(TestQuestError::StartUpError)?;

    // Spawn the main test pipeline consisting of three concurrent tasks:
    // - The test runner, which executes the HTTP requests.
    // - The asserter, which verifies the results.
    // - The outputter, which collects and displays final output.
    let (runner_jh, asserter_jh, outputter_handle) =
        run_pipeline_tasks(test_groups, n_tests, &app_handle.pool.clone(), &cli.path).await;

    // Wait for all background tasks to complete and gracefully shut down
    // the database container and application process.
    cleanup_and_teardown(&app_handle.child, runner_jh, asserter_jh, outputter_handle).await;

    // If the -o flag was provided, print the full captured stdout and stderr
    // output from the application after all tests have finished running.
    if cli.app_output {
        print_app_output(&app_handle.child.output).await;
    }

    Ok(())
}
