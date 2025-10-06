#![allow(unused)]
#![allow(clippy::result_large_err)]

use std::error::Error;

use clap::Parser;
use miette::Diagnostic;
use miette::IntoDiagnostic;
use miette::NamedSource;
use miette::Report;
use miette::Result;
use thiserror::Error;

use crate::asserter::Asserter;
use crate::cli::Cli;
use crate::parser::Proff;
use crate::runner::Runner;
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

fn main() -> Result<()> {
    try_main()
}

fn try_main() -> Result<()> {
    let cli = Cli::parse();

    let contents = std::fs::read_to_string(&cli.path).map_err(ProffError::FileError)?;
    let proff: Proff = toml::from_str(&contents).map_err(ProffError::TomlParsing)?;

    let tests =
        Validator::validate(&proff, &contents, &cli.path).map_err(ProffError::ValidationError)?;

    let runner = Runner::new(tests);
    let result = runner.start();

    let Ok(result) = result else { todo!() };

    let out_put = Asserter::run(result).map_err(|_| ProffError::AssertError)?;

    println!("{:#?}", out_put);

    Ok(())
}
