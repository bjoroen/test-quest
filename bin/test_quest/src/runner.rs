#![allow(clippy::enum_variant_names)]

use flume::SendError;
use flume::Sender;
use reqwest::Client;
use reqwest::Response;
use reqwest::StatusCode;
use reqwest::header::HeaderMap;
use sqlx::AnyPool;
use sqlx::Pool;
use sqlx::Row;
use thiserror::Error;
use url::Url;

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
    pool: Pool<sqlx::Any>,
) -> Result<(), RunnerError> {
    let client = Client::new();

    for test_group in ir.tests {
        let tx = tx.clone();
        let client = client.clone();
        let pool = pool.clone();

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

            if let Some(sql_statements) = &test.before_run {
                run_sql(&pool, sql_statements).await?
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
pub async fn run_sql_assertions(assertions: &mut [Assertion], pool: &AnyPool) {
    for ass in assertions.iter_mut() {
        if let Assertion::Sql { query, got, .. } = ass {
            let got_str = match sqlx::query(query).fetch_all(pool).await {
                Ok(rows) => {
                    let values: Vec<String> = rows
                        .iter()
                        .map(|row| {
                            if let Ok(s) = row.try_get::<String, _>(0) {
                                s
                            } else if let Ok(i) = row.try_get::<i64, _>(0) {
                                i.to_string()
                            } else if let Ok(f) = row.try_get::<f64, _>(0) {
                                f.to_string()
                            } else if let Ok(b) = row.try_get::<bool, _>(0) {
                                b.to_string()
                            } else if row.try_get::<Option<String>, _>(0).ok().flatten().is_none() {
                                "null".to_string()
                            } else {
                                "<unsupported type>".to_string()
                            }
                        })
                        .collect();

                    values.join(", ")
                }
                Err(e) => format!("SQL error: {e}"),
            };

            *got = Some(got_str);
        }
    }
}

async fn run_sql(pool: &AnyPool, sql_statements: &Vec<String>) -> Result<(), RunnerError> {
    for sql in sql_statements {
        sqlx::query(sql.as_str())
            .execute(pool)
            .await
            .map_err(RunnerError::DatabaseError)?;
    }

    Ok(())
}

pub async fn reset_database(pool: &AnyPool) -> Result<(), sqlx::Error> {
    let mut conn = pool.acquire().await?;

    // Try SQLite schema
    let sqlite_tables_res = sqlx::query(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
    )
    .try_map(|row: sqlx::any::AnyRow| row.try_get::<String, _>("name"))
    .fetch_all(&mut *conn)
    .await;

    let tables: Vec<String> = match sqlite_tables_res {
        Ok(t) if !t.is_empty() => t,
        _ => {
            sqlx::query(
                "SELECT table_name::text AS table_name
                 FROM information_schema.tables
                 WHERE table_schema = current_schema()
                 AND table_type = 'BASE TABLE'",
            )
            .try_map(|row: sqlx::any::AnyRow| row.try_get::<String, _>("table_name"))
            .fetch_all(&mut *conn)
            .await?
        }
    };

    for table in tables {
        match sqlx::query(&format!("TRUNCATE TABLE {table} CASCADE"))
            .execute(&mut *conn)
            .await
        {
            Ok(_) => { /* truncated successfully */ }
            Err(_) => {
                sqlx::query(&format!("DELETE FROM {table}"))
                    .execute(&mut *conn)
                    .await?;
            }
        }

        // Postgres sequence reset
        let reset_seq_sql_pg = format!(
            "DO $$ BEGIN IF EXISTS (SELECT 1 FROM pg_class WHERE relname = '{table}_id_seq') THEN \
             EXECUTE 'ALTER SEQUENCE {table}_id_seq RESTART WITH 1'; END IF; END $$;"
        );
        let _ = sqlx::query(&reset_seq_sql_pg).execute(&mut *conn).await;
    }

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
