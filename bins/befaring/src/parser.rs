use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Befaring {
    pub setup: Setup,
    pub db: Db,
    #[serde(default)]
    pub before_each: Option<Hook>,
    #[serde(default)]
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
    #[serde(default)]
    pub reset: Option<bool>,
    #[serde(default)]
    pub run_sql: Option<Vec<String>>,
}

// Group-level definition
#[derive(Deserialize, Debug, Clone)]
pub struct TestGroup {
    pub name: String,
    #[serde(default)]
    pub before_each: Option<Hook>, // Optional group-specific hook
    #[serde(default)]
    pub tests: Vec<Test>, // Tests in this group
    #[serde(default)]
    pub subgroups: Vec<TestGroup>, // Optional nested groups
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
