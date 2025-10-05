use std::collections::HashMap;
use std::str::FromStr;

use reqwest::Method;
use reqwest::Url;
use thiserror::Error;
use toml::Value;

use crate::parser::Proff;

pub struct Validator;

#[derive(Debug, Clone)]
pub enum Assertions {
    Status(i32),
    Headers(HashMap<String, String>),
}

pub struct Test {
    pub name: String,
    pub method: reqwest::Method,
    pub url: Url,
    pub body: Option<serde_json::Value>,
    pub assertions: Vec<Assertions>,
}

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Failed to read toml file")]
    HeaderAssertionError,
}

impl Validator {
    pub fn validate(proff: &Proff) -> Result<Vec<Test>, ValidationError> {
        let mut tests = vec![];
        for test in &proff.tests {
            let method = parse_method(&test.method);
            let name = test.name.clone();

            // TODO: This should return an error with trace to the file
            let url = Url::from_str(&format!("{}{}", proff.setup.url, &test.url)).unwrap();

            let body = test.body.clone();

            let assertions = parse_assertions(&test.assert_status, &test.assert_headers)?;

            tests.push(Test {
                name,
                method,
                url,
                body,
                assertions,
            });
        }

        Ok(tests)
    }
}

fn parse_assertions(
    assert_status: &Option<i32>,
    assert_headers: &Option<toml::Value>,
) -> Result<Vec<Assertions>, ValidationError> {
    let mut assert_vec = vec![];

    if let Some(status) = assert_status {
        assert_vec.push(Assertions::Status(*status));
    }

    if let Some(value) = assert_headers {
        let mut header_map = HashMap::new();
        match value {
            Value::Table(map) => {
                for (k, v) in map {
                    match v.as_str() {
                        Some(v) => header_map.insert(k.clone(), v.to_string()),
                        None => todo!(),
                    };
                }
            }
            _ => todo!(),
        }

        assert_vec.push(Assertions::Headers(header_map))
    }

    Ok(assert_vec)
}

fn parse_method(method: &str) -> reqwest::Method {
    match method.to_uppercase().as_str() {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "DELETE" => Method::DELETE,
        "PATCH" => Method::PATCH,
        "HEAD" => Method::HEAD,
        "OPTIONS" => Method::OPTIONS,
        _ => todo!("Need to handle error"),
    }
}
