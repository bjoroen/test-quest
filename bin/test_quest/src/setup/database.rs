use std::path::Path;

use sqlx::Any;
use sqlx::migrate::Migrator;
use testcontainers::ContainerAsync;
use testcontainers::ImageExt;
use testcontainers::TestcontainersError;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mariadb::Mariadb;
use testcontainers_modules::mysql::Mysql;
use testcontainers_modules::postgres::Postgres;
use thiserror::Error;

use crate::parser::ImageRef;

const POSTGRES: &str = "postgres";
const MYSQL: &str = "mysql";
const MARIADB: &str = "mariadb";

const POSTGRES_DEFAULT_TAG: &str = "16-alpine";

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Failed to start database container {0}")]
    TestContainer(#[from] TestcontainersError),

    #[error("We do not support this DB type")]
    UnknownDb,

    #[error("database failed with error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("faild to run migrations: {0}")]
    MigrationError(#[from] sqlx::migrate::MigrateError),

    #[error("timedout while waiting for database to be ready")]
    DatabaseTimeout,

    #[error("Failed to load initial sql {0}")]
    InitSql(#[from] std::io::Error),
}

/// Represents a running test container for a specific database type.
pub enum DatabaseContainer {
    Postgres(ContainerAsync<Postgres>),
    Mysql(ContainerAsync<Mysql>),
    Mariadb(ContainerAsync<Mariadb>),
}

/// Holds a running database container and its connection URL.
pub struct Database {
    pub database_container: DatabaseContainer,
    pub database_url: String,
}

/// Creates a `Database` instance for the specified database type.
///
/// This function starts a test container for the chosen database (`Postgres`,
/// `MySQL`, or `MariaDB`), determines the host port (using the provided
/// `db_port` or the default), and constructs the appropriate connection URL for
/// that container.
///
/// # Arguments
///
/// * `db_type` - The type of database to start (`"postgres"`, `"mysql"`,
///   `"mariadb"`).
/// * `db_port` - Optional port to bind the database to on localhost.
///
/// # Returns
///
/// Returns a `Database` struct containing the running container and the
/// connection URL.
///
/// # Errors
///
/// Returns `DbError::UnknownDb` if the database type is unrecognized, or
/// `DbError::TestContainer` if starting the container fails.
pub async fn from_type(
    db_type: String,
    db_port: Option<u16>,
    image_ref: Option<ImageRef>,
) -> Result<Database, DbError> {
    let database_container = match db_type.as_str() {
        POSTGRES => {
            let container = image_ref.map_or_else(
                || Postgres::default().with_tag(POSTGRES_DEFAULT_TAG),
                |image_ref| {
                    Postgres::default()
                        .with_tag(POSTGRES_DEFAULT_TAG)
                        .with_name(image_ref.name)
                        .with_tag(image_ref.tag)
                },
            );

            DatabaseContainer::Postgres(container.start().await.map_err(DbError::TestContainer)?)
        }
        MYSQL => DatabaseContainer::Mysql(
            Mysql::default()
                .start()
                .await
                .map_err(DbError::TestContainer)?,
        ),
        MARIADB => DatabaseContainer::Mariadb(
            Mariadb::default()
                .start()
                .await
                .map_err(DbError::TestContainer)?,
        ),
        _ => return Err(DbError::UnknownDb),
    };

    // TODO: The unwrap_or dont work, make it so you can set the hostname, mapped
    // port yourself
    let (host_port, host) = match &database_container {
        DatabaseContainer::Postgres(c) => (
            c.get_host_port_ipv4(db_port.unwrap_or(5432)).await?,
            c.get_host().await?,
        ),
        DatabaseContainer::Mysql(c) => (
            c.get_host_port_ipv4(db_port.unwrap_or(3306)).await?,
            c.get_host().await?,
        ),
        DatabaseContainer::Mariadb(c) => (
            c.get_host_port_ipv4(db_port.unwrap_or(3306)).await?,
            c.get_host().await?,
        ),
    };

    let database_url = match &database_container {
        DatabaseContainer::Postgres(_) => format!(
            "postgres://postgres:postgres@{}:{}/postgres",
            host, host_port
        ),
        DatabaseContainer::Mysql(_) => {
            format!("mysql://root:password@{}:{}", host, host_port)
        }
        DatabaseContainer::Mariadb(_) => {
            format!("mysql://root:password@{}:{}", host, host_port)
        }
    };

    Ok(Database {
        database_container,
        database_url,
    })
}

/// Establishes a database connection using a generic `Any` pool.
/// This allows connecting to any supported database type, determined at
/// runtime.
pub async fn connection_pool(database_url: &str) -> Result<sqlx::Pool<Any>, DbError> {
    // MANDATORY: When using `sqlx::AnyPool`, we must explicitly install the
    // compiled-in database drivers (PostgreSQL, MySQL, SQLite, etc.) at
    // runtime. If this is not called, using the `Any` pool will result in
    // a panic.
    //
    // Documentation: https://docs.rs/sqlx/latest/sqlx/any/fn.install_default_drivers.html
    sqlx::any::install_default_drivers();

    sqlx::Pool::<sqlx::Any>::connect(database_url)
        .await
        .map_err(DbError::DatabaseError)
}

/// Runs database migrations from the specified directory using a generic `Any`
/// pool.
pub async fn run_migrations(pool: &sqlx::Pool<Any>, migration_dir: &str) -> Result<(), DbError> {
    let migration_path = Path::new(migration_dir);

    let m = Migrator::new(migration_path)
        .await
        .map_err(DbError::MigrationError)?;

    m.run(pool).await.map_err(DbError::MigrationError)?;

    Ok(())
}

pub async fn load_init_sql(
    pool: &sqlx::Pool<Any>,
    path: std::path::PathBuf,
) -> Result<(), DbError> {
    let sql = std::fs::read_to_string(path).map_err(DbError::InitSql)?;

    sqlx::raw_sql(sql.as_str())
        .execute(pool)
        .await
        .map_err(DbError::DatabaseError)?;

    Ok(())
}

/// Waits for the database to become available by repeatedly executing a simple
/// query. Retries up to 30 times with a 500ms delay between attempts, returning
/// an error if the database does not respond within that timeframe.
pub async fn wait_for_db(database_pool: &sqlx::Pool<sqlx::Any>) -> Result<(), DbError> {
    for _ in 0..30 {
        if sqlx::query("SELECT 1").execute(database_pool).await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    Err(DbError::DatabaseTimeout)
}
