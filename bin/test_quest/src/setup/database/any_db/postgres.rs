use chrono::DateTime;
use chrono::NaiveDate;
use chrono::NaiveDateTime;
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::Column;
use uuid::Uuid;

use crate::setup::database::any_db::AnyRow;
use crate::setup::database::any_db::DbValue;

impl From<sqlx::postgres::PgRow> for AnyRow {
    fn from(row: sqlx::postgres::PgRow) -> Self {
        use sqlx::Row;

        let mut values = Vec::with_capacity(row.len());

        for col in row.columns() {
            let name = col.name();
            let typ = col.type_info().to_string();

            let value = match typ.as_str() {
                "INT2" => row
                    .try_get::<i16, _>(name)
                    .map(|v| DbValue::I64(v as i64))
                    .unwrap_or(DbValue::Null),
                "INT4" => row
                    .try_get::<i32, _>(name)
                    .map(|v| DbValue::I64(v as i64))
                    .unwrap_or(DbValue::Null),
                "INT8" => row
                    .try_get::<i64, _>(name)
                    .map(DbValue::I64)
                    .unwrap_or(DbValue::Null),
                "FLOAT4" => row
                    .try_get::<f32, _>(name)
                    .map(|v| DbValue::F64(v as f64))
                    .unwrap_or(DbValue::Null),
                "FLOAT8" => row
                    .try_get::<f64, _>(name)
                    .map(DbValue::F64)
                    .unwrap_or(DbValue::Null),
                "BOOL" => row
                    .try_get::<bool, _>(name)
                    .map(DbValue::Bool)
                    .unwrap_or(DbValue::Null),
                "TEXT" | "VARCHAR" | "CHAR" => row
                    .try_get::<String, _>(name)
                    .map(DbValue::String)
                    .unwrap_or(DbValue::Null),
                "UUID" => row
                    .try_get::<Uuid, _>(name)
                    .map(DbValue::Uuid)
                    .unwrap_or(DbValue::Null),
                "NUMERIC" => row
                    .try_get::<Decimal, _>(name)
                    .map(DbValue::Decimal)
                    .unwrap_or(DbValue::Null),
                "JSON" | "JSONB" => row
                    .try_get::<serde_json::Value, _>(name)
                    .map(DbValue::Json)
                    .unwrap_or(DbValue::Null),
                "BYTEA" => row
                    .try_get::<Vec<u8>, _>(name)
                    .map(DbValue::Bytes)
                    .unwrap_or(DbValue::Null),
                "DATE" => row
                    .try_get::<NaiveDate, _>(name)
                    .map(DbValue::Date)
                    .unwrap_or(DbValue::Null),
                "TIMESTAMP" => row
                    .try_get::<NaiveDateTime, _>(name)
                    .map(DbValue::DateTime)
                    .unwrap_or(DbValue::Null),
                "TIMESTAMPTZ" => row
                    .try_get::<DateTime<Utc>, _>(name)
                    .map(DbValue::Timestamp)
                    .unwrap_or(DbValue::Null),
                _ => DbValue::Unsupported,
            };

            values.push(value);
        }

        Self { values }
    }
}

#[cfg(test)]
mod test {
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use serde_json::json;
    use sqlx::Executor;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::setup::database;
    use crate::setup::database::any_db::DbValue;

    #[tokio::test]
    async fn postgres_type_test() {
        let database = database::from_type("postgres".into(), None, None)
            .await
            .unwrap();

        let sqlx_pool = sqlx::Pool::<sqlx::Postgres>::connect(&database.database_url)
            .await
            .unwrap();
        let any_pool = database::connection_pool(&database.database_url)
            .await
            .unwrap();

        let result = setup_test_table(&sqlx_pool);
        assert!(result.await.is_ok());
        let any_pool_all = any_pool.raw_sql("SELECT * FROM all_types").await.unwrap();

        assert_eq!(any_pool_all.len(), 1);
        assert!(
            any_pool_all.iter().all(|v| v
                .values
                .iter()
                .all(|v| !matches!(v, DbValue::Null) && !matches!(v, DbValue::Unsupported))),
            "Vec contains a Null or Unsupported value!"
        );
    }

    pub async fn setup_test_table(pool: &PgPool) -> sqlx::Result<()> {
        pool.execute(
            r#"
        DROP TABLE IF EXISTS all_types;
        CREATE TABLE all_types (
            id SERIAL PRIMARY KEY,
            smallint_col SMALLINT,
            integer_col INTEGER,
            bigint_col BIGINT,
            real_col REAL,
            double_col DOUBLE PRECISION,
            bool_col BOOLEAN,
            text_col TEXT,
            varchar_col VARCHAR(50),
            date_col DATE,
            timestamp_col TIMESTAMP,
            timestamptz_col TIMESTAMPTZ,
            uuid_col UUID,
            jsonb_col JSONB,
            bytea_col BYTEA,
            numeric_col NUMERIC
        );
        "#,
        )
        .await?;

        let uuid = Uuid::new_v4();
        let now = Utc::now();
        let date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let ts = NaiveDateTime::new(date, chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap());
        let json_val = json!({"key": "value"});

        #[allow(clippy::approx_constant)]
        sqlx::query(
            r#"
        INSERT INTO all_types (
            smallint_col, integer_col, bigint_col, real_col, double_col,
            bool_col, text_col, varchar_col, date_col, timestamp_col,
            timestamptz_col, uuid_col, jsonb_col, bytea_col, numeric_col
        ) VALUES (
            $1, $2, $3, $4, $5,
            $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15
        )
        "#,
        )
        .bind(16_i16)
        .bind(42_i32)
        .bind(1337_i64)
        .bind(3.14_f32)
        .bind(2.71828_f64)
        .bind(true)
        .bind("hello text")
        .bind("hello varchar")
        .bind(date)
        .bind(ts)
        .bind(now)
        .bind(uuid)
        .bind(json_val)
        .bind(vec![1_u8, 2, 3, 4])
        .bind(Decimal::new(12345, 2))
        .execute(pool)
        .await?;

        Ok(())
    }
}
