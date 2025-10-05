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
    pub fn new(tests: Vec<Test>) -> Self {
        let client = Client::new();

        Self {
            tests,
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
            let body = test.body.clone();
            let url = test.url.clone();
            let method = test.method.clone();
            let client = self.client.clone();
            let name = test.name.clone();
            let assertions = test.assertions.clone();

            let handle = task::spawn(async move {
                let request = match method.as_str() {
                    "POST" => {
                        if let Some(json_body) = body {
                            client.post(url).body(json_body.to_string())
                        } else {
                            client.post(url)
                        }
                    }
                    "PUT" => {
                        if let Some(json_body) = body {
                            client.put(url).body(json_body.to_string())
                        } else {
                            client.put(url)
                        }
                    }
                    "PATCH" => {
                        if let Some(json_body) = body {
                            client.patch(url).body(json_body.to_string())
                        } else {
                            client.patch(url)
                        }
                    }
                    "DELETE" => {
                        if let Some(json_body) = body {
                            client.delete(url).body(json_body.to_string())
                        } else {
                            client.delete(url)
                        }
                    }
                    "GET" => {
                        if let Some(json_body) = body {
                            client.get(url).body(json_body.to_string())
                        } else {
                            client.get(url)
                        }
                    }
                    // TODO:
                    // Move all parsing logic into parser, runner should never fail because of
                    // parser errors, like bad urls or methods types
                    _ => todo!(),
                };

                let request = request.build().unwrap();
                let result = client.execute(request).await;

                RunnerResult {
                    name,
                    request: result,
                    assertions,
                }
            });

            handles.push(handle);
        }

        let results = join_all(handles).await;
        let mut runner_results = vec![];
        for result in results {
            match result {
                Ok(result) => runner_results.push(result),
                Err(e) => eprintln!("Task failed: {:?}", e),
            }
        }

        Ok(runner_results)
    }
}
