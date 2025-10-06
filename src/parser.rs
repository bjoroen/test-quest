use miette::Diagnostic;
use miette::NamedSource;
use miette::SourceSpan;
use reqwest::Url;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use thiserror::Error;

#[derive(Deserialize, Debug)]
pub struct Proff {
    pub setup: Setup,
    pub tests: Vec<Test>,
}

#[derive(Deserialize, Debug)]
pub struct Setup {
    pub mode: String,
    pub base_url: String,
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
