#![allow(clippy::result_large_err)]

use std::sync::Arc;

use clap::Parser;
use console::Emoji;
use console::Style;
use console::Term;
use flume::Receiver;
use miette::Diagnostic;
use miette::Result;
use thiserror::Error;

use crate::asserter::AssertResult;
use crate::asserter::Asserter;
use crate::asserter::TestResult;
use crate::cli::Cli;
use crate::parser::Proff;
use crate::runner::RunnerResult;
use crate::runner::run_http_tests;
use crate::validator::ValidationError;
use crate::validator::Validator;

mod asserter;
mod cli;
mod parser;
mod runner;
mod validator;

#[derive(Error, Debug, Diagnostic)]
pub enum ProffError {
    #[error("Failed to read toml file")]
    FileError(#[from] std::io::Error),

    #[error("Failed to parse toml file")]
    TomlParsing(#[from] toml::de::Error),

    #[error(transparent)]
    #[diagnostic(transparent)]
    ValidationError(#[from] ValidationError),

    #[error("Failed in assert step")]
    AssertError,
}

struct OutPutter;

impl OutPutter {
    pub async fn start(
        rx: Receiver<(String, Arc<[AssertResult]>)>,
        test_path: &str,
        n_tests: usize,
    ) {
        let style = Style::new().bold().cyan();
        let open_text =
            &format!("Running test file: {test_path} Found {n_tests} tests: Running...");
        let open_text = style.apply_to(open_text);

        println!("{open_text}");
        let mut i = 1;
        let mut failed_tests: Vec<(String, AssertResult)> = vec![];
        while let Ok((name, result)) = rx.recv_async().await {
            for r in result.iter() {
                match r.status {
                    TestResult::Pass => {
                        println!(
                            "[{i}/{n_tests}] {}  {name}: {} {}",
                            console::style("✔").green().bold(),
                            r.actual,
                            console::style("PASS!").green().bold(),
                        )
                    }
                    TestResult::Fail => {
                        failed_tests.push((name.clone(), r.clone()));
                        println!(
                            "[{i}/{n_tests}] {}  {name}: {} {}",
                            console::style("╳").red().bold(),
                            r.expected,
                            console::style("FAILED!").red().bold(),
                        )
                    }
                }
            }

            i += 1;
        }

        if !failed_tests.is_empty() {
            println!();
            println!(
                "{}",
                console::style("Summary of Failed Tests:").bold().red()
            );
            for (idx, result) in failed_tests.iter().enumerate() {
                println!("\n{} {}. {}", idx + 1, result.0, result.1);
            }
        } else {
            println!();
            println!("{}", console::style("All tests passed! 🎉").bold().green());
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let (tx, rx) = flume::unbounded::<RunnerResult>();
    let (outputter_tx, outputter_rx) = flume::unbounded::<(String, Arc<[AssertResult]>)>();

    let contents = std::fs::read_to_string(&cli.path).map_err(ProffError::FileError)?;
    let proff: Proff = toml::from_str(&contents).map_err(ProffError::TomlParsing)?;

    let mut validator = Validator::new();

    let tests = validator
        .validate(&proff, &contents, &cli.path)
        .map_err(ProffError::ValidationError)?;

    let n_tests = tests.tests.len();

    let outputter_rx_printter = outputter_rx.clone();
    let outputter_handle = tokio::spawn(async move {
        OutPutter::start(outputter_rx_printter, &cli.path, n_tests).await;
    });

    let runner_jh = tokio::spawn(async move { run_http_tests(tests.tests, tx).await });

    let asserter_outputter_tx = outputter_tx.clone();
    let asserter_jh = tokio::spawn(async move { Asserter::run(rx, asserter_outputter_tx).await });

    drop(outputter_tx);
    let _ = futures::join!(runner_jh, asserter_jh, outputter_handle);

    Ok(())
}
