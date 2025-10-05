#![allow(unused)]
use clap::Parser;
use thiserror::Error;

use crate::asserter::Asserter;
use crate::cli::Cli;
use crate::parser::Proff;
use crate::runner::Runner;
use crate::validator::Validator;

mod asserter;
mod cli;
mod parser;
mod runner;
mod validator;

#[derive(Error, Debug)]
pub enum ProffError {
    #[error("Failed to read toml file")]
    FileError(#[from] std::io::Error),

    #[error("Failed to parse toml file")]
    TomlParsing(#[from] toml::de::Error),

    #[error("Failed to validate toml file")]
    ValidationError,

    #[error("Failed in assert step")]
    AssertError,
}

fn main() -> Result<(), ProffError> {
    let cli = Cli::parse();

    let contents = std::fs::read_to_string(cli.path).map_err(ProffError::FileError)?;
    let proff: Proff = toml::from_str(&contents).map_err(ProffError::TomlParsing)?;

    let tests = Validator::validate(&proff).map_err(|_| ProffError::ValidationError)?;

    let runner = Runner::new(tests);
    let result = runner.start();

    let Ok(result) = result else { todo!() };

    let out_put = Asserter::run(result).map_err(|_| ProffError::AssertError)?;

    println!("{:#?}", out_put);

    Ok(())
}
