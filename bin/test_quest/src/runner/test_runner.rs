#![allow(clippy::enum_variant_names)]

use std::sync::Arc;

use flume::Sender;
use reqwest::Client;
use url::Url;

use crate::runner::CapturedResponse;
use crate::runner::RunnerError;
use crate::runner::handle_before;
use crate::runner::run_sql_assertions;
use crate::setup::database::any_db::AnyDbPool;
use crate::validator::Assertion;
use crate::validator::TestGroups;

#[derive(Debug)]
pub struct RunnerResult {
    pub name: String,
    pub method: String,
    pub url: Url,
    pub response: Option<CapturedResponse>,
    pub error: Option<String>,
    pub assertions: Vec<Assertion>,
}

pub async fn run_tests(
    tests: Vec<TestGroups>,
    tx: Sender<RunnerResult>,
    pool: Arc<AnyDbPool>,
) -> Result<(), RunnerError> {
    let client = Client::new();

    for test_group in tests {
        let tx = tx.clone();
        let client = client.clone();

        handle_before(&pool, &test_group.before_group).await?;

        for mut test in test_group.tests {
            let client = client.clone();
            let tx = tx.clone();
            let url = test.url.clone();
            let method = test.method.to_string().clone();

            handle_before(&pool, &test.before_run).await?;

            let result = if let Some(body) = test.body {
                client
                    .request(test.method, url)
                    .headers(test.headers)
                    .json(&body)
            } else {
                client.request(test.method, url).headers(test.headers)
            }
            .send()
            .await;

            run_sql_assertions(&mut test.assertions, &pool).await;

            let runner_result = match result {
                Ok(resp) => RunnerResult {
                    name: test.name,
                    method,
                    url: test.url.clone(),
                    response: Some(CapturedResponse::from_response(resp).await),
                    error: None,
                    assertions: test.assertions,
                },
                Err(err) => RunnerResult {
                    name: test.name,
                    method,
                    url: test.url,
                    response: None,
                    error: Some(err.to_string()),
                    assertions: test.assertions,
                },
            };

            if let Err(error) = tx.send_async(runner_result).await {
                todo!("{error}")
            }
        }
    }
    Ok(())
}
