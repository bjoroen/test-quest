#![allow(clippy::enum_variant_names)]

use flume::SendError;
use flume::Sender;
use reqwest::Client;
use reqwest::Error;
use reqwest::Response;
use sqlx::AnyPool;
use sqlx::Pool;
use sqlx::Row;
use thiserror::Error;

use crate::validator::Assertion;
use crate::validator::IR;

#[derive(Error, Debug)]
pub enum RunnerError {
    #[error("channel error")]
    ChannelError(#[from] SendError<RunnerResult>),

    #[error("database error")]
    DatabaseError(#[from] sqlx::Error),
}

#[derive(Debug)]
pub struct RunnerResult {
    pub name: String,
    pub request: Result<Response, Error>,
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
                let Ok(_) = reset_database(&pool).await else {
                    todo!();
                };
            }

            if let Some(sql_statements) = &before.sql {
                run_sql(&pool, sql_statements).await?
            }
        }

        for mut test in test_group.tests {
            let client = client.clone();
            let tx = tx.clone();

            if let Some(sql_statements) = &test.before_run {
                run_sql(&pool, sql_statements).await?
            }

            let result = if let Some(body) = test.body {
                client
                    .request(test.method, test.url)
                    .headers(test.headers)
                    // Should not be hardcoded that its json
                    .json(&body)
            } else {
                client.request(test.method, test.url).headers(test.headers)
            }
            .send()
            .await;

            run_sql_assertions(&mut test.assertions, &pool).await;

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

    // Try SQLite master first
    let sqlite_tables_res: Result<Vec<String>, sqlx::Error> = sqlx::query(
        r#"
        SELECT name FROM sqlite_master
        WHERE type='table' AND name NOT LIKE 'sqlite_%'
        "#,
    )
    .try_map(|row: sqlx::any::AnyRow| row.try_get::<String, _>("name"))
    .fetch_all(&mut *conn)
    .await;

    // Use sqlite result if ok, otherwise try information_schema fallback
    let tables: Vec<String> = match sqlite_tables_res {
        Ok(t) if !t.is_empty() => t,
        _ => {
            // Fallback for Postgres/MySQL/MariaDB
            sqlx::query(
                r#"
                SELECT table_name AS name
                FROM information_schema.tables
                WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
                "#,
            )
            .try_map(|row: sqlx::any::AnyRow| row.try_get::<String, _>("name"))
            .fetch_all(&mut *conn)
            .await?
        }
    };

    for table in tables {
        // Try to TRUNCATE first, fallback to DELETE if not supported
        let truncate_sql = format!("TRUNCATE TABLE {table}");
        if sqlx::query(&truncate_sql)
            .execute(&mut *conn)
            .await
            .is_err()
        {
            let delete_sql = format!("DELETE FROM {table}");
            sqlx::query(&delete_sql).execute(&mut *conn).await?;
        }

        // Try to reset sequences (for Postgres/MySQL)
        let reset_seq_sql = format!("ALTER TABLE {table} AUTO_INCREMENT = 1");
        let _ = sqlx::query(&reset_seq_sql).execute(&mut *conn).await;
    }

    Ok(())
}
