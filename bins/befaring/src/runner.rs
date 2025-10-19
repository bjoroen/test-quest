#![allow(clippy::enum_variant_names)]

use flume::SendError;
use flume::Sender;
use reqwest::Client;
use reqwest::Error;
use reqwest::Response;
use sqlx::Pool;
use thiserror::Error;
use tokio::task;

use crate::validator::Assertion;
use crate::validator::IR;

#[derive(Error, Debug)]
pub enum RunnerError {
    #[error("channel error")]
    ChannelError(#[from] SendError<RunnerResult>),
}

#[derive(Debug)]
pub struct RunnerResult {
    pub name: String,
    pub request: Result<Response, Error>,
    pub assertions: Vec<Assertion>,
}

pub async fn run_http_tests(
    ir: IR,
    tx: Sender<RunnerResult>,
    _pool: Pool<sqlx::Any>,
) -> Result<(), RunnerError> {
    let client = Client::new();

    for test_group in &ir.tests {
        test_group.tests.clone().into_iter().for_each(|test| {
            let client = client.clone();
            let tx = tx.clone();

            task::spawn(async move {
                let result = if let Some(body) = test.body {
                    client.request(test.method, test.url).body(body.to_string())
                } else {
                    client.request(test.method, test.url)
                }
                .send()
                .await;

                if let Err(error) = tx
                    .send_async(RunnerResult {
                        name: test.name,
                        request: result,
                        assertions: test.assertions,
                    })
                    .await
                {
                    todo!("{error}")
                }
            });
        });
    }
    Ok(())
}
