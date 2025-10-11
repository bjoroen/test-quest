#![allow(clippy::enum_variant_names)]

use std::time::Duration;

use flume::SendError;
use flume::Sender;
use futures::future::join_all;
use reqwest::Client;
use reqwest::Error;
use reqwest::Method;
use reqwest::Request;
use reqwest::RequestBuilder;
use reqwest::Response;
use reqwest::Url;
use thiserror::Error;
use tokio::task;

use crate::Stage;
use crate::parser::Proff;
use crate::validator::Assertions;
use crate::validator::IR;
use crate::validator::Test;

#[derive(Error, Debug)]
pub enum RunnerError<'a> {
    #[error("interna error")]
    InternalError,

    #[error("Run error: {0}")]
    RunError(&'a str),

    #[error("channel error")]
    ChannelError(#[from] SendError<RunnerResult>),

    #[error("channel error")]
    OutputterChannelError(#[from] SendError<(i32, Stage)>),
}

#[derive(Debug)]
pub struct RunnerResult {
    pub name: String,
    pub request: Result<Response, Error>,
    pub assertions: Vec<Assertions>,
}

pub struct Runner {
    tests: Vec<Test>,
    client: reqwest::Client,
    results: Option<RunnerResult>,
}

impl Runner {
    pub fn new(ir: IR) -> Self {
        let client = Client::new();

        Self {
            tests: ir.tests,
            client,
            results: None,
        }
    }

    pub async fn run(
        self,
        tx: Sender<RunnerResult>,
        outputter_tx: Sender<(i32, Stage)>,
    ) -> Result<(), RunnerError<'static>> {
        let handles: Vec<_> = self
            .tests
            .into_iter()
            .map(|test| {
                let client = self.client.clone();

                let tx = tx.clone();
                let outputter_tx = outputter_tx.clone();

                task::spawn(async move {
                    outputter_tx
                        .send_async((test.id, Stage::Registrated))
                        .await
                        .map_err(RunnerError::OutputterChannelError)?;

                    #[cfg(feature = "slow")]
                    {
                        tokio::time::sleep(Duration::from_secs(1));
                    }

                    let result = if let Some(body) = test.body {
                        client.request(test.method, test.url).body(body.to_string())
                    } else {
                        client.request(test.method, test.url)
                    }
                    .send()
                    .await;

                    outputter_tx
                        .send_async((test.id, Stage::Running))
                        .await
                        .map_err(RunnerError::OutputterChannelError)?;

                    #[cfg(feature = "slow")]
                    {
                        tokio::time::sleep(Duration::from_secs(1));
                    }

                    tx.send_async(RunnerResult {
                        name: test.name,
                        request: result,
                        assertions: test.assertions,
                    })
                    .await
                    .map_err(RunnerError::ChannelError)?;

                    Ok(())
                })
            })
            .collect();

        futures::future::join_all(handles)
            .await
            .into_iter()
            .filter_map(|r| match r {
                Ok(res) => Some(res),
                Err(e) => {
                    eprintln!("Task failed: {:?}", e);
                    None
                }
            })
            .collect::<Result<(), RunnerError>>();

        Ok(())
    }
}
