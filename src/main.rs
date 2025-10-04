#![allow(unused)]
use clap::Parser;
use thiserror::Error;

use crate::asserter::Asserter;
use crate::cli::Cli;
use crate::parser::Proff;
use crate::runner::Runner;
use crate::validator::Validator;

mod cli;

#[derive(Error, Debug)]
pub enum ProffError {
    #[error("Failed to read toml file")]
    FileError(#[from] std::io::Error),

    #[error("Failed to parse toml file")]
    TomlParsing(#[from] toml::de::Error),
}

fn main() -> Result<(), ProffError> {
    let cli = Cli::parse();

    let contents = std::fs::read_to_string(cli.path).map_err(ProffError::FileError)?;
    let proff: Proff = toml::from_str(&contents).map_err(ProffError::TomlParsing)?;

    let tests = Validator::validate(&proff);

    let runner = Runner::new(tests);
    let result = runner.start();

    let Ok(result) = result else { todo!() };

    let out_put = Asserter::run(result);

    Ok(())
}

mod asserter {
    use crate::runner::RunnerResult;

    pub struct Asserter {}
    pub struct Output {}
    pub enum OutputError {}

    impl Asserter {
        pub fn run(runner_results: Vec<RunnerResult>) -> Result<Output, OutputError> {
            todo!()
        }
    }
}

mod validator {
    use std::str::FromStr;

    use reqwest::Url;

    use crate::parser::Proff;

    pub struct Validator;
    #[derive(Debug, Clone)]
    pub enum Assertions {
        Status(i32),
    }

    pub struct Test {
        pub name: String,
        pub method: reqwest::Method,
        pub url: Url,
        pub body: Option<serde_json::Value>,
        pub assertions: Vec<Assertions>,
    }

    impl Validator {
        pub fn validate(proff: &Proff) -> Vec<Test> {
            for test in &proff.tests {
                let method = parse_method(&test.method);
                let name = test.name.clone();
                // TODO: This should return an error with trace to the file
                let url = Url::from_str(&format!("{}{}", proff.setup.url, &test.url)).unwrap();
                let body = test.body.clone();

                Test {
                    name,
                    method,
                    url,
                    body,
                    assertions: todo!(),
                };
            }

            todo!()
        }
    }

    fn parse_method(method: &str) -> reqwest::Method {
        todo!()
    }
}

mod runner {
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
        name: String,
        request: Result<Response, Error>,
        assertions: Vec<Assertions>,
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
}

mod parser {

    use reqwest::Url;
    use serde::Deserialize;
    use serde::Deserializer;
    use serde::Serialize;

    #[derive(Deserialize, Debug)]
    pub struct Proff {
        pub setup: Setup,
        pub tests: Vec<Test>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Setup {
        pub mode: String,
        pub url: String,
    }

    #[derive(Deserialize, Debug)]
    pub struct Test {
        pub name: String,
        pub method: String,
        pub url: String,
        #[serde(default)]
        pub body: Option<serde_json::Value>,
        pub assert_status: Option<i32>,
    }
}
