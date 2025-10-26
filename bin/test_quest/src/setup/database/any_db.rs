use std::fmt::Display;
use std::path::Path;

use chrono::DateTime;
use chrono::NaiveDate;
use chrono::NaiveDateTime;
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::Executor;
use sqlx::migrate::Migrator;
use uuid::Uuid;

pub mod mysql;
pub mod postgres;

#[derive(Debug, PartialEq)]
pub enum DbValue {
    I64(i64),
    F64(f64),
    Bool(bool),
    String(String),
    Bytes(Vec<u8>),
    Decimal(Decimal),
    Uuid(Uuid),
    Json(serde_json::Value),
    Date(NaiveDate),
    DateTime(NaiveDateTime),
    Timestamp(DateTime<Utc>),
    Null,
    Unsupported,
}

impl Display for DbValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbValue::I64(v) => write!(f, "{}", v),
            DbValue::F64(v) => write!(f, "{}", v),
            DbValue::Bool(v) => write!(f, "{}", v),
            DbValue::String(v) => write!(f, "{}", v),
            DbValue::Bytes(v) => write!(f, "{:?}", v),
            DbValue::Decimal(v) => write!(f, "{}", v),
            DbValue::Uuid(v) => write!(f, "{}", v),
            DbValue::Json(v) => write!(f, "{}", v),
            DbValue::Date(v) => write!(f, "{}", v),
            DbValue::DateTime(v) => write!(f, "{}", v),
            DbValue::Timestamp(v) => write!(f, "{}", v),
            DbValue::Null => write!(f, "NULL"),
            DbValue::Unsupported => write!(f, "<unsupported>"),
        }
    }
}

#[derive(Debug)]
pub struct AnyRow {
    pub values: Vec<DbValue>,
}
impl AnyRow {
    pub fn to_csv_line(&self) -> String {
        self.values
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }
}

pub enum AnyDbPool {
    Postgres(sqlx::Pool<sqlx::Postgres>),
    MySql(sqlx::Pool<sqlx::MySql>),
}

impl AnyDbPool {
    pub async fn raw_sql(&self, query: &str) -> Result<Vec<AnyRow>, sqlx::Error> {
        match self {
            AnyDbPool::Postgres(pool) => {
                let rows = pool.fetch_all(query).await.unwrap();
                Ok(rows.into_iter().map(Into::into).collect())
            }
            AnyDbPool::MySql(pool) => {
                let rows = pool.fetch_all(query).await.unwrap();
                Ok(rows.into_iter().map(Into::into).collect())
            }
        }
    }
    pub async fn migrate(&self, migration_path: &Path) -> Result<(), sqlx::migrate::MigrateError> {
        let m = Migrator::new(Path::new(migration_path)).await?;

        match self {
            AnyDbPool::Postgres(pool) => {
                m.run(pool).await?;
            }
            AnyDbPool::MySql(pool) => {
                m.run(pool).await?;
            }
        }

        Ok(())
    }
}
