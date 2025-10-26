use std::collections::HashMap;
use std::fmt;

use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct TestQuest {
    pub setup: Setup,
    pub db: Db,
    pub before_each_group: Option<Hook>,
    pub test_groups: Vec<TestGroup>,
    pub global: Global,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Global {
    pub headers: Option<toml::Value>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Db {
    pub db_type: String,
    pub migration_dir: String,
    pub port: Option<u16>,
    pub init_sql: Option<String>,
    pub image_ref: Option<ImageRef>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct ImageRef {
    pub name: String,
    pub tag: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Setup {
    pub base_url: String,
    pub command: String,
    pub args: Option<Vec<String>>,
    pub ready_when: String,
    pub database_url_env: Option<String>,
    pub env: Option<HashMap<String, String>>,
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
#[serde(untagged)]
pub enum StringOrStrings {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Deserialize, Clone)]
pub struct AssertSql {
    pub query: String,
    pub expect: StringOrStrings,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Test {
    pub before_run: Option<Hook>,
    pub name: String,
    pub method: String,
    pub headers: Option<toml::Value>,
    pub url: String,
    pub query: Option<String>,
    pub body: Option<serde_json::Value>,
    pub assert_status: Option<i32>,
    pub assert_headers: Option<toml::Value>,
    pub assert_db_state: Option<AssertSql>,
    pub assert_json: Option<serde_json::Value>,
}

impl fmt::Display for StringOrStrings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StringOrStrings::Single(s) => write!(f, "{s}"),
            StringOrStrings::Multiple(v) => {
                if v.is_empty() {
                    write!(f, "[]")
                } else if v.len() == 1 {
                    write!(f, "[{}]", v[0])
                } else {
                    write!(f, "[{}]", v.join(", "))
                }
            }
        }
    }
}
