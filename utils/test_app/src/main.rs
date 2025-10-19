use std::env;
use std::net::SocketAddr;

use axum::Json;
use axum::Router;
use axum::extract::Path;
use axum::extract::Request;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::delete;
use axum::routing::get;
use axum::routing::patch;
use axum::routing::post;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use sqlx::Row;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct User {
    id: i64,
    name: String,
    password: String,
}

async fn middleware_fn(
    // run the `HeaderMap` extractor
    headers: HeaderMap,
    // you can also add more extractors here but the last
    // extractor must implement `FromRequest` which
    // `Request` does
    request: Request,
    next: Next,
) -> Response {
    println!("{:#?}", headers);
    println!("{:#?}", request);

    next.run(request).await
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // read DATABASE_URL from env
    let database_url =
        env::var("DATABASE_URL").expect("DATABASE_URL must be set in environment variables");

    // initialize Postgres pool
    let pool = PgPool::connect(&database_url).await?;

    // build router and inject pool as shared state
    let app = Router::new()
        .route("/health", get(health))
        .route("/login", post(login))
        .route("/login/password/change", patch(change_password))
        .route("/ready", get(ready))
        .route("/users/{id}", delete(delete_user))
        .route("/users", post(create_user))
        .route("/users", get(list_users))
        .route("/users/{id}", get(get_user))
        // .layer(axum::middleware::from_fn(middleware_fn))
        .with_state(pool);

    // run app
    let addr = SocketAddr::from(([127, 0, 0, 1], 6969));
    println!("🚀 API running at http://{addr}");

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:6969").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

async fn change_password(
    State(pool): State<PgPool>,
    Json(payload): Json<LoginRequest>,
) -> impl IntoResponse {
    let name = &payload.username;
    let password = &payload.password;

    sqlx::query("UPDATE users SET password = $1 WHERE name = $2")
        .bind(password)
        .bind(name)
        .execute(&pool)
        .await
        .inspect_err(|e| println!("{e}"))
        .expect("Failed to insert user");

    StatusCode::OK
}

async fn login(State(pool): State<PgPool>, Json(payload): Json<LoginRequest>) -> impl IntoResponse {
    let name = &payload.username;
    let password = &payload.password;

    let row = sqlx::query("SELECT * from users where name = $1;")
        .bind(name)
        .fetch_one(&pool)
        .await
        .expect("Failed to insert user");

    if row.get::<String, _>("password") != *password {
        return StatusCode::UNAUTHORIZED;
    }

    StatusCode::OK
}

async fn health() -> &'static str {
    "ok"
}

async fn ready() -> &'static str {
    "ok"
}

async fn delete_user(
    Path(user_id): Path<i32>,
    State(pool): State<PgPool>,
) -> Result<(StatusCode, Json<User>), (StatusCode, String)> {
    let row = sqlx::query("DELETE FROM users WHERE id = $1 RETURNING id, name, password")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::NOT_FOUND,
                format!("Failed to delete user: {}", e),
            )
        })?;

    let user = User {
        id: row.get("id"),
        name: row.get("name"),
        password: row.get("password"),
    };

    Ok((StatusCode::OK, Json(user)))
}

async fn create_user(State(pool): State<PgPool>) -> (StatusCode, Json<User>) {
    let row = sqlx::query(
        "INSERT INTO users (id, name, password) VALUES ($1, $2, $3) RETURNING id, name, password",
    )
    .bind(11)
    .bind("new_name")
    .bind("password")
    .fetch_one(&pool)
    .await
    .expect("Failed to insert user");

    let user = User {
        id: row.get("id"),
        name: row.get("name"),
        password: row.get("password"),
    };

    (StatusCode::CREATED, Json(user))
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

    Json(serde_json::json!(users))
}

async fn get_user(
    Path(id): Path<i64>,
    State(pool): State<PgPool>,
) -> Result<Json<User>, StatusCode> {
    let row = sqlx::query("SELECT id, name, password FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(User {
        id: row.get("id"),
        name: row.get("name"),
        password: row.get("password"),
    }))
}
