#![allow(unused)]
#![allow(clippy::result_large_err)]

use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

use clap::Parser;
use flume::Receiver;
use miette::Diagnostic;
use miette::IntoDiagnostic;
use miette::NamedSource;
use miette::Report;
use miette::Result;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::task;

use crate::asserter::Asserter;
use crate::cli::Cli;
use crate::parser::Proff;
use crate::runner::Runner;
use crate::runner::RunnerResult;
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

#[derive(Debug)]
enum Stage {
    Registrated,
    Running,
    Asserting,
    Done,
}

struct OutPutter {
    tests: Arc<Mutex<HashMap<i32, Stage>>>,
}

impl OutPutter {
    pub fn new() -> Self {
        Self {
            tests: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn start(&self, rx: Receiver<(i32, Stage)>) {
        let tests = Arc::clone(&self.tests);

        let rx_task = task::spawn(async move {
            while let Ok(msg) = rx.recv_async().await {
                let mut map = tests.lock().await;
                println!("{} - {:#?}", msg.0, msg.1);
                map.insert(msg.0, msg.1);
            }
        })
        .await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let (tx, rx) = flume::unbounded::<RunnerResult>();
    let (outputter_tx, outputter_rx) = flume::unbounded::<(i32, Stage)>();

    let outputter_handle = tokio::spawn(async move {
        let outputter = OutPutter::new().start(outputter_rx).await;
    });

    let contents = std::fs::read_to_string(&cli.path).map_err(ProffError::FileError)?;
    let proff: Proff = toml::from_str(&contents).map_err(ProffError::TomlParsing)?;

    let mut validator = Validator::new();

    let tests = validator
        .validate(&proff, &contents, &cli.path)
        .map_err(ProffError::ValidationError)?;

    let n_tests = tests.tests.len();

    let runner_output_tx = outputter_tx.clone();
    let runner_fut = Runner::new(tests).run(tx, runner_output_tx);

    let asserter_outputter_tx = outputter_tx.clone();
    let asserter_fut = Asserter::run(rx, asserter_outputter_tx);
    let (runner_result, out_put) = futures::join!(runner_fut, asserter_fut);

    println!("{:#?}", out_put);

    Ok(())
}
