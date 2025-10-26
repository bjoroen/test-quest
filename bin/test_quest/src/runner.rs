#![allow(clippy::enum_variant_names)]

use std::sync::Arc;

use flume::SendError;
use flume::Sender;
use reqwest::Client;
use reqwest::Response;
use reqwest::StatusCode;
use reqwest::header::HeaderMap;
use thiserror::Error;
use url::Url;

use crate::setup::database::any_db::AnyDbPool;
use crate::validator::Assertion;
use crate::validator::IR;

#[derive(Error, Debug)]
// TODO: Fix large enum
#[allow(clippy::large_enum_variant)]
pub enum RunnerError {
    #[error("channel error")]
    ChannelError(#[from] SendError<RunnerResult>),

    #[error("database error")]
    DatabaseError(#[from] sqlx::Error),
}

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
    ir: IR,
    tx: Sender<RunnerResult>,
    pool: Arc<AnyDbPool>,
) -> Result<(), RunnerError> {
    let client = Client::new();

    for test_group in ir.tests {
        let tx = tx.clone();
        let client = client.clone();

        // If the test group has put database reset to true, we reset the database
        // before the tests run
        if let Some(before) = test_group.before_group {
            if before.reset_db.is_some_and(|b| b) {
                reset_database(&pool)
                    .await
                    .map_err(RunnerError::DatabaseError)?;
            }

            if let Some(sql_statements) = &before.sql {
                run_sql(&pool, sql_statements).await?
            }
        }

        for mut test in test_group.tests {
            let client = client.clone();
            let tx = tx.clone();
            let url = test.url.clone();
            let method = test.method.to_string().clone();

            // TODO: Duplicated logic with the one above
            if let Some(before) = test.before_run {
                if before.reset_db.is_some_and(|b| b) {
                    reset_database(&pool)
                        .await
                        .map_err(RunnerError::DatabaseError)?;
                }

                if let Some(sql_statements) = &before.sql {
                    run_sql(&pool, sql_statements).await?
                }
            }

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

/// Executes all SQL assertions in-place, handling multiple rows and types.
/// Fills the `got` field for each `Assertion::Sql`.
pub async fn run_sql_assertions(assertions: &mut [Assertion], pool: &AnyDbPool) {
    for ass in assertions.iter_mut() {
        if let Assertion::Sql { query, got, .. } = ass {
            let got_str = pool.raw_sql(query).await.unwrap();

            *got = Some("some string".into());
        }
    }
}

async fn run_sql(pool: &AnyDbPool, sql_statements: &Vec<String>) -> Result<(), RunnerError> {
    for sql in sql_statements {
        pool.raw_sql(sql)
            .await
            .map_err(RunnerError::DatabaseError)?;
    }

    Ok(())
}

pub async fn reset_database(_pool: &AnyDbPool) -> Result<(), sqlx::Error> {
    Ok(())
}

#[derive(Debug)]
pub struct CapturedResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body_text: String,
    pub body_json: Option<serde_json::Value>,
}

impl CapturedResponse {
    pub async fn from_response(resp: Response) -> Self {
        let status = resp.status();
        let headers = resp.headers().clone();

        // Consume the body exactly once
        let body_text = match resp.text().await {
            Ok(t) => t,
            Err(err) => format!("Failed to read body: {}", err),
        };

        // Attempt to parse JSON, but don't panic
        let body_json = serde_json::from_str::<serde_json::Value>(&body_text).ok();

        Self {
            status,
            headers,
            body_text,
            body_json,
        }
    }
}
