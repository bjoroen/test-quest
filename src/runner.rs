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

use crate::parser::Proff;
use crate::validator::Assertions;
use crate::validator::IR;
use crate::validator::Test;

#[derive(Error, Debug)]
pub enum RunnerError<'a> {
    #[error("internal error")]
    InternalError,

    #[error("Run error: {0}")]
    RunError(&'a str),
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

    pub fn start(&self) -> Result<Vec<RunnerResult>, RunnerError<'_>> {
        match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|_| RunnerError::InternalError)?
            .block_on(self.run())
        {
            Ok(r) => Ok(r),
            Err(e) => Err(e),
        }
    }

    async fn run(&self) -> Result<Vec<RunnerResult>, RunnerError<'_>> {
        let mut handles = vec![];

        for test in &self.tests {
            let client = self.client.clone();

            let body = test.body.clone();
            let url = test.url.clone();
            let method = test.method.clone();
            let name = test.name.clone();
            let assertions = test.assertions.clone();

            let handle = task::spawn(async move {
                let result = if let Some(body) = body {
                    client.request(method, url).body(body.to_string())
                } else {
                    client.request(method, url)
                }
                .send()
                .await;

                RunnerResult {
                    name,
                    request: result,
                    assertions,
                }
            });

            handles.push(handle);
        }

        let results = join_all(handles).await;

        let runner_results = results
            .into_iter()
            .filter_map(|r| match r {
                Ok(res) => Some(res),
                Err(e) => {
                    eprintln!("Task failed: {:?}", e);
                    None
                }
            })
            .collect();

        Ok(runner_results)
    }
}
