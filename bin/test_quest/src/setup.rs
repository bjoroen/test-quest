use std::sync::Arc;

use database::Database;
use thiserror::Error;

use crate::setup::app::AppError;
use crate::setup::app::AppProcess;
use crate::setup::database::DatabaseContainer;
use crate::setup::database::DbError;
use crate::setup::database::db::AnyDbPool;
use crate::validator::EnvSetup;

pub mod app;
pub mod database;

pub struct AppHandle {
    pub child: AppProcess,
    pub database_container: DatabaseContainer,
    pub pool: Arc<AnyDbPool>,
}

#[derive(Debug, Error)]
pub enum StartUpError {
    #[error("Start up process failed with database errror: {0}")]
    DatabaseError(DbError),

    #[error("Start up process failed with App Error: {0}")]
    AppError(AppError),

    #[error("Failed to connect with app: {0}")]
    AppTimeout(AppError),
}

pub async fn start_db_and_app(
    env_setup: EnvSetup,
    stream_app: bool,
) -> Result<AppHandle, StartUpError> {
    // Get all values needed from the
    // `env_setup`
    let EnvSetup {
        base_url,
        command,
        args,
        ready_when,
        db_type,
        migration_dir,
        db_port,
        database_url_env,
        init_sql,
        image_ref,
    } = env_setup;

    print_with_color("[SETUP] setting up database container! ⚙️");

    let Database {
        database_container,
        database_url,
    } = database::from_type(db_type, db_port, image_ref)
        .await
        .map_err(StartUpError::DatabaseError)?;

    print_with_color("[SETUP] connecting to database! ⚙️");

    let pool = database::connection_pool(&database_url)
        .await
        .map_err(StartUpError::DatabaseError)?;

    print_with_color("[SETUP] waiting for database to be ready..! ⚙️");

    if let Err(e) = database::wait_for_db(&pool).await {
        return Err(StartUpError::DatabaseError(e));
    };

    if let Some(migration_dir) = migration_dir {
        database::run_migrations(&pool, &migration_dir)
            .await
            .map_err(StartUpError::DatabaseError)?;
    };

    if let Some(path) = init_sql {
        print_with_color("[SETUP] loading init sql..! ⚙️");
        database::load_init_sql(&pool, path)
            .await
            .map_err(StartUpError::DatabaseError)?;
    };

    print_with_color("[SETUP] setting up app..! ⚙️");

    let child = app::from_command(command, args, database_url_env, database_url, stream_app)
        .await
        .map_err(StartUpError::AppError)?;

    print_with_color("[SETUP] waiting for app to be ready..! ⚙️");

    if let Err(error) = app::wait_for_app_ready(base_url.as_str(), ready_when.as_str()).await {
        let mut lock = child.process.lock().await;
        let _ = lock.kill().await;

        return Err(StartUpError::AppTimeout(error));
    }

    print_with_color("[SETUP] App is ready to rock and roll..! ⚙️");

    Ok(AppHandle {
        child,
        database_container,
        pool,
    })
}

fn print_with_color(s: &str) {
    println!("{}", console::style(s).bold().yellow());
}
