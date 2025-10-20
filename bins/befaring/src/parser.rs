use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Befaring {
    pub setup: Setup,
    pub db: Db,
    pub before_each_group: Option<Hook>,
    pub test_groups: Vec<TestGroup>,
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
pub struct Hook {
    pub reset: Option<bool>,
    pub run_sql: Option<Vec<String>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TestGroup {
    pub name: String,
    pub before_each_test: Option<Hook>,
    pub before_group: Option<Hook>,
    pub tests: Vec<Test>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AssertSql {
    pub query: String,
    pub expect: String,
}
#[derive(Deserialize, Debug, Clone)]
pub struct Test {
    pub before_run: Option<Vec<String>>,
    pub name: String,
    pub method: String,
    pub headers: Option<toml::Value>,
    pub url: String,
    pub body: Option<serde_json::Value>,
    pub assert_status: Option<i32>,
    pub assert_headers: Option<toml::Value>,
    pub assert_sql: Option<AssertSql>,
    pub assert_json: Option<serde_json::Value>,
}
