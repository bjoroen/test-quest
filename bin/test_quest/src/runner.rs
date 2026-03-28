use std::sync::Arc;

use flume::SendError;
use flume::Sender;
use reqwest::Response;
use reqwest::StatusCode;
use reqwest::header::HeaderMap;
use thiserror::Error;

use crate::runner::test_runner::RunnerResult;
use crate::setup::database::any_db::AnyDbPool;
use crate::validator::Assertion;
use crate::validator::BeforeEach;
use crate::validator::IR;
use crate::validator::TestGroups;
use crate::validator::prop_tests::PropTests;

pub mod prop_runner;
pub mod test_runner;

#[derive(Error, Debug)]
// TODO: Fix large enum
#[allow(clippy::large_enum_variant)]
pub enum RunnerError {
    #[error("channel error")]
    ChannelError(#[from] SendError<RunnerResult>),

    #[error("database error")]
    DatabaseError(#[from] sqlx::Error),
}

pub struct Runner;

impl Runner {
    pub async fn run(
        &self,
        ir: IR,
        tx: Sender<RunnerResult>,
        pool: Arc<AnyDbPool>,
    ) -> Result<(), RunnerError> {
        let IR {
            before_each_group: _,
            tests,
            prop_tests,
        } = ir;

        let tx_prop = tx.clone();
        let pool_prop = pool.clone();
        self.run_prop_tests(prop_tests, tx_prop, pool_prop).await?;

        let tx_test = tx.clone();
        let pool_test = pool.clone();
        self.run_tests(tests, tx_test, pool_test).await
    }

    async fn run_tests(
        &self,
        tests: Vec<TestGroups>,
        tx: Sender<RunnerResult>,
        pool: Arc<AnyDbPool>,
    ) -> Result<(), RunnerError> {
        test_runner::run_tests(tests, tx, pool).await
    }

    async fn run_prop_tests(
        &self,
        prop_tests: Option<Vec<PropTests>>,
        tx: Sender<RunnerResult>,
        pool: Arc<AnyDbPool>,
    ) -> Result<(), RunnerError> {
        if let Some(prop_tests) = prop_tests {
            prop_runner::run_prop_tests(prop_tests, tx, pool).await?
        }

        Ok(())
    }
}

pub async fn handle_before(
    pool: &AnyDbPool,
    before: &Option<BeforeEach>,
) -> Result<(), RunnerError> {
    if let Some(before) = before {
        if before.reset_db.is_some_and(|b| b) {
            reset_database(pool)
                .await
                .map_err(RunnerError::DatabaseError)?;
        }

        if let Some(sql_statements) = &before.sql {
            run_sql(pool, sql_statements).await?;
        }
    }

    Ok(())
}

/// Executes all SQL assertions in-place, handling multiple rows and types.
/// Fills the `got` field for each `Assertion::Sql`.
pub async fn run_sql_assertions(assertions: &mut [Assertion], pool: &AnyDbPool) {
    for ass in assertions.iter_mut() {
        if let Assertion::Sql { query, got, .. } = ass {
            let rows = pool.raw_sql(query).await.unwrap();

            let vec_of_colums: Vec<String> = rows.iter().map(|row| row.to_csv_line()).collect();
            *got = Some(vec_of_colums);
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
    pub body_text: Option<String>,
    pub body_json: Option<serde_json::Value>,
}

impl CapturedResponse {
    pub async fn from_response(resp: Response) -> Self {
        let status = resp.status();
        let headers = resp.headers().clone();

        let body_text = match resp.text().await {
            Ok(t) => t,
            Err(err) => format!("Failed to read body: {}", err),
        };

        let body_json = serde_json::from_str::<serde_json::Value>(&body_text).ok();

        Self {
            status,
            headers,
            body_text: Some(body_text),
            body_json,
        }
    }
}
