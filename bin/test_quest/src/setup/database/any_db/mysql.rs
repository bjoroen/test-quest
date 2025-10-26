use chrono::DateTime;
use chrono::NaiveDate;
use chrono::NaiveDateTime;
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::Column;

use crate::setup::database::any_db::AnyRow;
use crate::setup::database::any_db::DbValue;

impl From<sqlx::mysql::MySqlRow> for AnyRow {
    fn from(row: sqlx::mysql::MySqlRow) -> Self {
        use sqlx::Row;

        let mut values = Vec::with_capacity(row.len());

        for col in row.columns() {
            let name = col.name();
            let typ = col.type_info().to_string();

            let value = match typ.as_str() {
                // Integers
                "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "BIGINT" => row
                    .try_get::<i64, _>(name)
                    .map(DbValue::I64)
                    .unwrap_or(DbValue::Null),

                // Floating point
                "FLOAT" | "DOUBLE" => row
                    .try_get::<f64, _>(name)
                    .map(DbValue::F64)
                    .unwrap_or(DbValue::Null),

                "DECIMAL" => row
                    .try_get::<Decimal, _>(name)
                    .map(DbValue::Decimal)
                    .unwrap_or(DbValue::Null),

                // Boolean
                "BOOLEAN" | "BIT" => row
                    .try_get::<bool, _>(name)
                    .map(DbValue::Bool)
                    .unwrap_or(DbValue::Null),

                // Strings
                "CHAR" | "VARCHAR" | "TEXT" | "TINYTEXT" | "MEDIUMTEXT" | "LONGTEXT" => row
                    .try_get::<String, _>(name)
                    .map(DbValue::String)
                    .unwrap_or(DbValue::Null),

                // Binary
                "BLOB" | "TINYBLOB" | "MEDIUMBLOB" | "LONGBLOB" | "BINARY" | "VARBINARY" => row
                    .try_get::<Vec<u8>, _>(name)
                    .map(DbValue::Bytes)
                    .unwrap_or(DbValue::Null),

                // Dates / Times
                "DATE" => row
                    .try_get::<NaiveDate, _>(name)
                    .map(DbValue::Date)
                    .unwrap_or(DbValue::Null),
                "DATETIME" => row
                    .try_get::<NaiveDateTime, _>(name)
                    .map(DbValue::DateTime)
                    .unwrap_or(DbValue::Null),
                "TIMESTAMP" => row
                    .try_get::<DateTime<Utc>, _>(name)
                    .map(DbValue::Timestamp) // or DbValue::DateTime if you prefer
                    .unwrap_or(DbValue::Null),

                // JSON
                "JSON" => row
                    .try_get::<serde_json::Value, _>(name)
                    .map(DbValue::Json)
                    .unwrap_or(DbValue::Null),

                // Fallback
                _ => DbValue::Unsupported,
            };

            if value == DbValue::Null {
                dbg!(typ);
            }

            values.push(value);
        }

        Self { values }
    }
}

#[cfg(test)]
mod test {
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use rust_decimal::Decimal;
    use serde_json::json;
    use sqlx::Executor;
    use uuid::Uuid;

    use crate::setup::database::any_db::DbValue;
    use crate::setup::database::{self};

    #[tokio::test]
    async fn basic_test_mysql() {
        let database = database::from_type("mysql".into(), None, None)
            .await
            .unwrap();

        let sqlx_pool = sqlx::Pool::<sqlx::MySql>::connect(&database.database_url)
            .await
            .unwrap();

        let any_pool = database::connection_pool(&database.database_url)
            .await
            .unwrap();

        let result = setup_test_table_mysql(&sqlx_pool);
        assert!(result.await.is_ok());

        let any_pool_all = any_pool.raw_sql("SELECT * FROM all_types").await.unwrap();

        assert_eq!(any_pool_all.len(), 1);
        assert!(
            any_pool_all
                .iter()
                .all(|row| row.values.iter().all(|v| !matches!(v, DbValue::Null))),
            "Vec contains a Null value!"
        );
    }

    pub async fn setup_test_table_mysql(pool: &sqlx::MySqlPool) -> sqlx::Result<()> {
        // Drop & create table
        pool.execute(
            r#"
        DROP TABLE IF EXISTS all_types;
        CREATE TABLE all_types (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            tinyint_col TINYINT,
            smallint_col SMALLINT,
            int_col INT,
            bigint_col BIGINT,
            float_col FLOAT,
            double_col DOUBLE,
            bool_col BOOLEAN,
            text_col TEXT,
            varchar_col VARCHAR(50),
            date_col DATE,
            datetime_col DATETIME,
            timestamp_col TIMESTAMP,
            uuid_col CHAR(36),
            json_col JSON,
            blob_col BLOB,
            decimal_col DECIMAL(10,2)
        );
    "#,
        )
        .await?;

        let uuid = Uuid::new_v4();
        let date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let ts = NaiveDateTime::new(date, chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap());
        let json_val = json!({"key": "value"});

        #[allow(clippy::approx_constant)]
        sqlx::query(
            r#"
        INSERT INTO all_types (
            tinyint_col, smallint_col, int_col, bigint_col, float_col, double_col,
            bool_col, text_col, varchar_col, date_col, datetime_col, timestamp_col,
            uuid_col, json_col, blob_col, decimal_col
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    "#,
        )
        .bind(1_i8)
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
        .bind(ts)
        .bind(uuid.to_string())
        .bind(json_val)
        .bind(vec![1_u8, 2, 3, 4])
        .bind(Decimal::new(12345, 2))
        .execute(pool)
        .await?;

        Ok(())
    }
}
