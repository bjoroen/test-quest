use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Befaring {
    pub setup: Setup,
    pub db: Db,
    pub tests: Vec<Test>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Db {
    pub db_type: String,
    pub migration_dir: String,
    pub port: Option<u16>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Setup {
    pub base_url: String,
    pub command: String,
    pub args: Option<Vec<String>>,
    pub ready_when: String,
    pub database_url_env: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Test {
    pub name: String,
    pub method: String,
    pub url: String,
    pub body: Option<serde_json::Value>,
    pub assert_status: Option<i32>,
    pub assert_headers: Option<toml::Value>,
}
