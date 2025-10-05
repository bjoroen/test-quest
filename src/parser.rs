use reqwest::Url;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;

#[derive(Deserialize, Debug)]
pub struct Proff {
    pub setup: Setup,
    pub tests: Vec<Test>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Setup {
    pub mode: String,
    pub url: String,
}

#[derive(Deserialize, Debug)]
pub struct Test {
    pub name: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub body: Option<serde_json::Value>,
    pub assert_status: Option<i32>,
    pub assert_headers: Option<toml::Value>,
}
