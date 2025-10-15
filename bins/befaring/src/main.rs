#![allow(clippy::result_large_err)]
#![allow(dead_code)]

use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use miette::Diagnostic;
use miette::Result;
use reqwest::Client;
use sqlx::Pool;
use sqlx::pool;
use sqlx::postgres::PgPoolOptions;
use testcontainers::ContainerAsync;
use testcontainers::ImageExt;
use testcontainers::core::WaitFor;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres;
use testcontainers_modules::postgres::Postgres;
use thiserror::Error;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::asserter::AssertResult;
use crate::asserter::Asserter;
use crate::cli::Cli;
use crate::outputter::OutPutter;
use crate::parser::Befaring;
use crate::runner::RunnerResult;
use crate::runner::run_http_tests;
use crate::validator::ValidationError;
use crate::validator::Validator;

mod asserter;
mod cli;
mod outputter;
mod parser;
mod runner;
mod validator;

#[derive(Error, Debug, Diagnostic)]
pub enum BefaringError {
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

struct AppHandle {
    child: Arc<Mutex<tokio::process::Child>>,
    database_container: ContainerAsync<Postgres>,
}

async fn start_db_and_app() -> Result<AppHandle, ()> {
    let database_container = postgres::Postgres::default().start().await.unwrap();
    let host_port = database_container.get_host_port_ipv4(5432).await.unwrap();

    database_container.stdout(true);
    database_container.stderr(true);

    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{}", host_port);
    println!("Database URL: {}", database_url);

    if wait_for_db(&database_url).await.is_err() {
        panic!("DB timeout");
    };

    let child = Command::new("cargo")
        .args(["run", "-p", "test_app"])
        .env("DATABASE_URL", &database_url)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to spawn test_app");

    let child = Arc::new(Mutex::new(child));

    if (wait_for_app_ready("http://127.0.0.1:6969/health", 30).await).is_err() {
        let mut lock = child.lock().await;
        let _ = lock.kill().await;
        panic!("app not ready");
    }

    Ok(AppHandle {
        child,
        database_container,
    })
}

async fn wait_for_db(database_url: &str) -> Result<(), ()> {
    for _ in 0..30 {
        if let Ok(pool) = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            && sqlx::query("SELECT 1").execute(&pool).await.is_ok()
        {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    Err(())
}

async fn wait_for_app_ready(url: &str, timeout_secs: u64) -> Result<(), ()> {
    let client = Client::new();
    let mut elapsed = 0;

    while elapsed < timeout_secs {
        if let Ok(resp) = client.get(url).send().await
            && resp.status().is_success()
        {
            println!("App is ready!");
            return Ok(());
        }

        sleep(Duration::from_secs(1)).await;
        elapsed += 1;
    }

    Err(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let (tx, rx) = flume::unbounded::<RunnerResult>();
    let (outputter_tx, outputter_rx) = flume::unbounded::<(String, Arc<[AssertResult]>)>();

    let contents = std::fs::read_to_string(&cli.path).map_err(BefaringError::FileError)?;
    let befaring: Befaring = toml::from_str(&contents).map_err(BefaringError::TomlParsing)?;

    let mut validator = Validator::new();

    let tests = validator
        .validate(&befaring, &contents, &cli.path)
        .map_err(BefaringError::ValidationError)?;

    let n_tests = tests.tests.len();

    let db = befaring.db.clone();
    let setup_command = befaring.setup.command.clone();
    let app_handle = start_db_and_app().await.unwrap();

    let outputter_rx_printter = outputter_rx.clone();
    let outputter_handle = tokio::spawn(async move {
        OutPutter::start(outputter_rx_printter, &cli.path, n_tests).await;
    });

    let runner_jh = tokio::spawn(async move { run_http_tests(tests.tests, tx).await });

    let asserter_outputter_tx = outputter_tx.clone();
    let asserter_jh = tokio::spawn(async move { Asserter::run(rx, asserter_outputter_tx).await });

    drop(outputter_tx);
    let _ = futures::join!(runner_jh, asserter_jh, outputter_handle);
    let mut lock = app_handle.child.lock().await;
    let _ = lock.kill().await;

    Ok(())
}
