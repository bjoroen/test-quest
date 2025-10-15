use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Befaring {
    pub setup: Setup,
    pub db: Db,
    pub tests: Vec<Test>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Db {
    db_type: String,
    migrations: String,
    runtime: String,
}

#[derive(Deserialize, Debug)]
pub struct Setup {
    pub mode: String,
    pub base_url: String,
    pub command: String,
}

#[derive(Deserialize, Debug)]
pub struct Test {
    pub name: String,
    pub method: String,
    pub url: String,
    pub body: Option<serde_json::Value>,
    pub assert_status: Option<i32>,
    pub assert_headers: Option<toml::Value>,
}
