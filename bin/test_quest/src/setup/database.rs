use std::path::Path;
use std::sync::Arc;

use testcontainers::ContainerAsync;
use testcontainers::GenericImage;
use testcontainers::ImageExt;
use testcontainers::TestcontainersError;
use testcontainers::core::ContainerPort;
use testcontainers::runners::AsyncRunner;
use thiserror::Error;

use crate::parser::ImageRef;
use crate::setup::database::db::AnyDbPool;

const POSTGRES: &str = "postgres";
const MYSQL: &str = "mysql";
const MARIADB: &str = "mariadb";

const POSTGRES_DEFAULT_TAG: &str = "16-alpine";

pub mod db;

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
    Postgres(ContainerAsync<testcontainers_modules::postgres::Postgres>),
    Mysql(ContainerAsync<testcontainers::GenericImage>),
    Mariadb(ContainerAsync<testcontainers::GenericImage>),
}

/// Holds a running database container and its connection URL.
pub struct Database {
    pub database_container: DatabaseContainer,
    pub database_url: String,
}

struct DbLogger;

/// TODO: Update this comment
/// Creates a `Database` instance for the specified database type.
///
/// This fuction starts a test container for the chosen database (`Postgres`,
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
                || {
                    testcontainers_modules::postgres::Postgres::default()
                        .with_tag(POSTGRES_DEFAULT_TAG)
                },
                |image_ref| {
                    testcontainers_modules::postgres::Postgres::default()
                        .with_name(image_ref.name)
                        .with_tag(image_ref.tag)
                },
            );

            DatabaseContainer::Postgres(
                container
                    .with_env_var("POSTGRES_LOGGING_COLLECTOR", "on")
                    .with_env_var("POSTGRES_LOG_STATEMENT", "all")
                    .start()
                    .await
                    .map_err(DbError::TestContainer)?,
            )
        }
        MYSQL => {
            let container = image_ref.map_or_else(
                || GenericImage::new("mysql", "oraclelinux9"),
                |image_ref| GenericImage::new(image_ref.name, image_ref.tag),
            );

            DatabaseContainer::Mysql(
                container
                    .with_mapped_port(db_port.unwrap_or(3306), ContainerPort::Tcp(3306))
                    .with_network("bridge")
                    .start()
                    .await
                    .map_err(DbError::TestContainer)?,
            )
        }
        MARIADB => {
            let container = image_ref.map_or_else(
                || GenericImage::new("mmariadb", "lts-ubi9"),
                |image_ref| GenericImage::new(image_ref.name, image_ref.tag),
            );

            DatabaseContainer::Mariadb(
                container
                    .with_mapped_port(db_port.unwrap_or(3306), ContainerPort::Tcp(3306))
                    .with_network("bridge")
                    .start()
                    .await
                    .map_err(DbError::TestContainer)?,
            )
        }
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
pub async fn connection_pool(db_url: &str) -> Result<Arc<AnyDbPool>, DbError> {
    if db_url.starts_with("postgres://") {
        Ok(Arc::new(AnyDbPool::Postgres(
            sqlx::Pool::<sqlx::Postgres>::connect(db_url).await?,
        )))
    } else if db_url.starts_with("mysql://") {
        Ok(Arc::new(AnyDbPool::MySql(
            sqlx::Pool::<sqlx::MySql>::connect(db_url).await?,
        )))
    } else {
        panic!("Unsupported database type: {}", db_url);
    }
}

/// Runs database migrations from the specified directory using a generic `Any`
/// pool.
pub async fn run_migrations(pool: &AnyDbPool, migration_dir: &str) -> Result<(), DbError> {
    let migration_path = Path::new(migration_dir);

    pool.migrate(migration_path)
        .await
        .map_err(DbError::MigrationError)?;

    Ok(())
}

pub async fn load_init_sql(pool: &AnyDbPool, path: std::path::PathBuf) -> Result<(), DbError> {
    let sql = std::fs::read_to_string(path).map_err(DbError::InitSql)?;

    pool.raw_sql(&sql).await.map_err(DbError::DatabaseError)?;

    Ok(())
}

/// Waits for the database to become available by repeatedly executing a simple
/// query. Retries up to 30 times with a 500ms delay between attempts, returning
/// an error if the database does not respond within that timeframe.
pub async fn wait_for_db(pool: &AnyDbPool) -> Result<(), DbError> {
    for _ in 0..30 {
        if pool.raw_sql("SELECT 1").await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    Err(DbError::DatabaseTimeout)
}
