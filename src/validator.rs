use std::str::FromStr;

use miette::Diagnostic;
use miette::NamedSource;
use miette::SourceSpan;
use reqwest::Method;
use reqwest::Url;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use thiserror::Error;
use toml::Value;

use crate::parser::Proff;

pub struct Validator(i32);

#[derive(Debug, Clone)]
pub enum Assertion {
    Status(i32),
    Headers(HeaderMap),
}

pub struct IR {
    pub tests: Vec<Test>,
}
#[derive(Clone)]
pub struct Test {
    pub name: String,
    pub method: Method,
    pub url: Url,
    pub body: Option<serde_json::Value>,
    pub assertions: Vec<Assertion>,
}

#[derive(Debug, Error, Diagnostic)]
#[error("Invalid field `{field}`: {message}")]
pub struct ValidationError {
    field: String,
    message: String,
    #[source_code]
    src: Option<NamedSource<String>>,
    #[label("invalid value here")]
    span: Option<SourceSpan>,
}

fn find_span(needle: &str, toml_src: &str) -> Option<SourceSpan> {
    toml_src
        .find(needle)
        .map(|start| SourceSpan::new(start.into(), needle.len()))
}

impl Validator {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn validate(
        &mut self,
        proff: &Proff,
        toml_src: &str,
        file_name: &str,
    ) -> miette::Result<IR, ValidationError> {
        let tests: Vec<Test> = proff
            .tests
            .iter()
            .map(|test| {
                let method =
                    parse_method(&test.method.to_uppercase()).map_err(|e| ValidationError {
                        field: format!("{} - method", test.name),
                        message: e.to_string(),
                        src: Some(NamedSource::new(file_name, toml_src.to_string())),
                        span: find_span(&test.method, toml_src),
                    })?;

                let url = parse_url(&proff.setup.base_url, &test.url).map_err(|e| match e {
                    ParseUrlError::SetupUrlEndsWithSlash => ValidationError {
                        field: "setup.url".into(),
                        message: "The base URL from setup can’t end with a /, and each URL in \
                                  test must start with one"
                            .into(),
                        src: Some(NamedSource::new(file_name, toml_src.to_string())),
                        span: find_span(&proff.setup.base_url, toml_src),
                    },
                    ParseUrlError::PathUrlMissingSlash => ValidationError {
                        field: format!("{}/url", test.name),
                        message: "The URL field in a test is required to begin with a leading /."
                            .into(),
                        src: Some(NamedSource::new(file_name, toml_src.to_string())),
                        span: find_span(&test.url, toml_src),
                    },
                    ParseUrlError::ParseIntoUrlFailed(parse_error) => ValidationError {
                        field: format!("{}.url", &proff.setup.base_url),
                        message: parse_error.to_string(),
                        src: None,
                        span: None,
                    },
                })?;

                let body = test.body.clone();
                let name = test.name.clone();

                let assertions = parse_assertions(
                    &test.assert_status,
                    &test.assert_headers,
                    Some((file_name.into(), toml_src.into())),
                )?;

                Ok(Test {
                    name,
                    body,
                    method,
                    url,
                    assertions,
                })
            })
            .collect::<Result<_, _>>()?;

        Ok(IR { tests })
    }
}

#[derive(Debug, Error)]
enum ParseUrlError {
    #[error("")]
    SetupUrlEndsWithSlash,
    #[error("")]
    PathUrlMissingSlash,
    #[error("Failed to parse URL: {0}")]
    ParseIntoUrlFailed(#[from] url::ParseError),
}
fn parse_url(base_url: &str, path_url: &str) -> Result<Url, ParseUrlError> {
    if base_url.ends_with("/") {
        return Err(ParseUrlError::SetupUrlEndsWithSlash);
    }

    if !path_url.starts_with("/") {
        return Err(ParseUrlError::PathUrlMissingSlash);
    }

    let url = reqwest::Url::parse(&format!("{base_url}{path_url}"))
        .map_err(ParseUrlError::ParseIntoUrlFailed)?;

    Ok(url)
}

fn parse_method(method: &str) -> Result<reqwest::Method, String> {
    let method = Method::from_str(method).map_err(|e| e.to_string())?;

    if !matches!(
        method,
        Method::GET
            | Method::POST
            | Method::PUT
            | Method::DELETE
            | Method::PATCH
            | Method::HEAD
            | Method::OPTIONS
            | Method::CONNECT
            | Method::TRACE
    ) {
        return Err(format!("Invalid HTTP method: {}", method));
    }

    Ok(method)
}

pub fn parse_assertions(
    assert_status: &Option<i32>,
    assert_headers: &Option<Value>,
    src: Option<(String, String)>, // (filename, source contents)
) -> Result<Vec<Assertion>, ValidationError> {
    let mut assert_vec = vec![];

    if let Some(status) = assert_status {
        assert_vec.push(Assertion::Status(*status));
    }

    if let Some(value) = assert_headers {
        let mut header_map = HeaderMap::new();

        match value {
            Value::Table(map) => {
                for (k, v) in map {
                    let v_str = v.as_str().ok_or_else(|| ValidationError {
                        field: k.clone(),
                        message: format!("Header value must be a string, got {v:?}"),
                        src: src
                            .as_ref()
                            .map(|(name, content)| NamedSource::new(name.clone(), content.clone())),
                        span: find_key_span(src.as_ref(), k),
                    })?;

                    let name =
                        HeaderName::from_bytes(k.as_bytes()).map_err(|e| ValidationError {
                            field: k.clone(),
                            message: format!("Invalid header name `{k}`: {e}"),
                            src: src.as_ref().map(|(name, content)| {
                                NamedSource::new(name.clone(), content.clone())
                            }),
                            span: find_key_span(src.as_ref(), k),
                        })?;

                    let value = HeaderValue::from_str(v_str).map_err(|e| ValidationError {
                        field: k.clone(),
                        message: format!("Invalid header value for `{k}`: {e}"),
                        src: src
                            .as_ref()
                            .map(|(name, content)| NamedSource::new(name.clone(), content.clone())),
                        span: find_value_span(src.as_ref(), v_str),
                    })?;

                    header_map.insert(name, value);
                }
            }
            _ => {
                return Err(ValidationError {
                    field: "headers".to_string(),
                    message: format!("Expected a table for headers, got {value:?}"),
                    src: src
                        .as_ref()
                        .map(|(name, content)| NamedSource::new(name.clone(), content.clone())),
                    span: None,
                });
            }
        }

        assert_vec.push(Assertion::Headers(header_map));
    }

    Ok(assert_vec)
}
fn find_key_span(src: Option<&(String, String)>, key: &str) -> Option<SourceSpan> {
    let (_, content) = src?;
    let start = content.find(key)?;
    Some(SourceSpan::new(start.into(), key.len()))
}

fn find_value_span(src: Option<&(String, String)>, value: &str) -> Option<SourceSpan> {
    let (_, content) = src?;
    let start = content.find(value)?;
    Some(SourceSpan::new(start.into(), value.len()))
}
