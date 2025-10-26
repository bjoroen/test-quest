use std::path::Path;

use sqlx::migrate::Migrator;

pub trait DynRow: Send + Sync {
    fn try_get_i64(&self, idx: usize) -> Result<i64, sqlx::Error>;
    fn try_get_f64(&self, idx: usize) -> Result<f64, sqlx::Error>;
    fn try_get_bool(&self, idx: usize) -> Result<bool, sqlx::Error>;
    fn try_get_string(&self, idx: usize) -> Result<String, sqlx::Error>;

    fn try_get_optional_i64(&self, idx: usize) -> Result<Option<i64>, sqlx::Error>;
    fn try_get_optional_f64(&self, idx: usize) -> Result<Option<f64>, sqlx::Error>;
    fn try_get_optional_bool(&self, idx: usize) -> Result<Option<bool>, sqlx::Error>;
    fn try_get_optional_string(&self, idx: usize) -> Result<Option<String>, sqlx::Error>;

    fn len(&self) -> usize;
}

impl DynRow for sqlx::postgres::PgRow {
    fn try_get_i64(&self, idx: usize) -> Result<i64, sqlx::Error> {
        sqlx::Row::try_get(self, idx)
    }
    fn try_get_string(&self, idx: usize) -> Result<String, sqlx::Error> {
        sqlx::Row::try_get(self, idx)
    }

    fn try_get_f64(&self, idx: usize) -> Result<f64, sqlx::Error> {
        sqlx::Row::try_get(self, idx)
    }

    fn try_get_bool(&self, idx: usize) -> Result<bool, sqlx::Error> {
        sqlx::Row::try_get(self, idx)
    }

    fn try_get_optional_i64(&self, idx: usize) -> Result<Option<i64>, sqlx::Error> {
        sqlx::Row::try_get(self, idx)
    }

    fn try_get_optional_f64(&self, idx: usize) -> Result<Option<f64>, sqlx::Error> {
        sqlx::Row::try_get(self, idx)
    }

    fn try_get_optional_bool(&self, idx: usize) -> Result<Option<bool>, sqlx::Error> {
        sqlx::Row::try_get(self, idx)
    }

    fn try_get_optional_string(&self, idx: usize) -> Result<Option<String>, sqlx::Error> {
        sqlx::Row::try_get(self, idx)
    }

    fn len(&self) -> usize {
        sqlx::Row::len(self)
    }
}

impl DynRow for sqlx::mysql::MySqlRow {
    fn try_get_i64(&self, idx: usize) -> Result<i64, sqlx::Error> {
        sqlx::Row::try_get(self, idx)
    }
    fn try_get_string(&self, idx: usize) -> Result<String, sqlx::Error> {
        sqlx::Row::try_get(self, idx)
    }

    fn try_get_f64(&self, idx: usize) -> Result<f64, sqlx::Error> {
        sqlx::Row::try_get(self, idx)
    }

    fn try_get_bool(&self, idx: usize) -> Result<bool, sqlx::Error> {
        sqlx::Row::try_get(self, idx)
    }

    fn try_get_optional_i64(&self, idx: usize) -> Result<Option<i64>, sqlx::Error> {
        sqlx::Row::try_get(self, idx)
    }

    fn try_get_optional_f64(&self, idx: usize) -> Result<Option<f64>, sqlx::Error> {
        sqlx::Row::try_get(self, idx)
    }

    fn try_get_optional_bool(&self, idx: usize) -> Result<Option<bool>, sqlx::Error> {
        sqlx::Row::try_get(self, idx)
    }

    fn try_get_optional_string(&self, idx: usize) -> Result<Option<String>, sqlx::Error> {
        sqlx::Row::try_get(self, idx)
    }

    fn len(&self) -> usize {
        todo!()
    }
}

pub enum AnyDbPool {
    Postgres(sqlx::Pool<sqlx::Postgres>),
    MySql(sqlx::Pool<sqlx::MySql>),
}

impl AnyDbPool {
    pub async fn raw_sql(&self, query: &str) -> Result<Vec<Box<dyn DynRow>>, sqlx::Error> {
        match self {
            AnyDbPool::Postgres(pool) => {
                let rows = sqlx::query(query).fetch_all(pool).await?;
                Ok(rows
                    .into_iter()
                    .map(|r| Box::new(r) as Box<dyn DynRow>)
                    .collect())
            }
            AnyDbPool::MySql(pool) => {
                let rows = sqlx::query(query).fetch_all(pool).await?;
                Ok(rows
                    .into_iter()
                    .map(|r| Box::new(r) as Box<dyn DynRow>)
                    .collect())
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
