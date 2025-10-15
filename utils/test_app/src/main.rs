use std::env;
use std::net::SocketAddr;
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;
use tokio::time::sleep;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct User {
    id: i64,
    name: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // read DATABASE_URL from env
    let database_url =
        env::var("DATABASE_URL").expect("DATABASE_URL must be set in environment variables");

    // initialize Postgres pool

    let pool = PgPoolOptions::new()
        .max_connections(10) // increase from default 5
        .connect(&database_url)
        .await?;

    let _ = dbg!(setup_database(&pool).await);

    // build router and inject pool as shared state
    let app = Router::new()
        .route("/health", get(health))
        .route("/users", get(list_users))
        .route("/users/{id}", get(get_user))
        .with_state(pool.clone());

    // run app
    let addr = SocketAddr::from(([127, 0, 0, 1], 6969));
    println!("🚀 API running at http://{addr}");

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:6969").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn create_user(
    Json(payload): Json<User>,
    State(pool): State<PgPool>,
) -> (StatusCode, Json<User>) {
    let row = sqlx::query("INSERT INTO users (id, name) VALUES ($1, $2) RETURNING id, name")
        .bind(payload.id)
        .bind(&payload.name)
        .fetch_one(&pool)
        .await
        .expect("Failed to insert user");

    let user = User {
        id: row.get("id"),
        name: row.get("name"),
    };

    (StatusCode::OK, Json(user))
}

async fn list_users(State(pool): State<PgPool>) -> Json<Value> {
    let rows = sqlx::query("SELECT id, name FROM users")
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch users");

    let users: Vec<Value> = rows
        .into_iter()
        .map(|r| serde_json::json!({ "id": r.get::<i64,_>("id"), "name": r.get::<String,_>("name") }))
        .collect();

    dbg!(&users);

    Json(serde_json::json!(users))
}

async fn get_user(
    Path(id): Path<i64>,
    State(pool): State<PgPool>,
) -> Result<Json<User>, StatusCode> {
    let row = sqlx::query("SELECT id, name FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(User {
        id: row.get("id"),
        name: row.get("name"),
    }))
}

async fn setup_database(pool: &PgPool) -> anyhow::Result<()> {
    // 1️⃣ Create the table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id BIGINT PRIMARY KEY,
            name TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    // 2️⃣ Insert users individually or in a single multi-row insert
    sqlx::query(
        r#"
        INSERT INTO users (id, name) VALUES ($1, $2)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(1i64)
    .bind("Alice")
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO users (id, name) VALUES ($1, $2)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(2i64)
    .bind("Bob")
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO users (id, name) VALUES ($1, $2)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(3i64)
    .bind("Charlie")
    .execute(pool)
    .await?;

    Ok(())
}
