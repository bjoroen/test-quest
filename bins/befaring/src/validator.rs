use std::str::FromStr;

use miette::Diagnostic;
use miette::NamedSource;
use miette::SourceSpan;
use reqwest::Method;
use reqwest::Url;
use reqwest::header::HeaderMap;
use thiserror::Error;

mod parser_assertion;

use crate::parser;
use crate::parser::Befaring;
use crate::parser::Hook;

// Error messages for parsing URLs
const BASE_URL_ENDS_WITH: &str =
    "The base URL from setup can’t end with a /, and each URL in test must start with one";
const PATH_URL_MISSING_SLASH: &str =
    "The URL field in a test is required to begin with a leading /.";

pub struct Validator {
    befaring: Befaring,
    toml_src: String,
    file_name: String,
}

#[derive(Debug, Clone)]
pub enum Assertion {
    Status(i32),
    Headers(HeaderMap),
}

pub struct EnvSetup {
    pub base_url: String,
    pub command: String,
    pub args: Option<Vec<String>>,
    pub ready_when: String,
    pub db_type: String,
    pub migration_dir: Option<String>,
    pub db_port: Option<u16>,
    pub database_url_env: String,
}

pub struct IR {
    pub before_each: Option<BeforeEach>,
    pub tests: Vec<TestGroups>,
}

pub struct TestGroups {
    pub name: String,
    pub before_each: Option<BeforeEach>,
    pub tests: Vec<ValidatedTests>,
}

pub struct BeforeEach {
    pub reset_db: Option<bool>,
    pub sql: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct ValidatedTests {
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

macro_rules! validation_err {
    ($field:expr, $msg:expr, $self:expr, $snippet:expr) => {
        ValidationError {
            field: $field.to_string(),
            message: $msg.to_string(),
            src: Some(NamedSource::new(
                $self.file_name.clone(),
                $self.toml_src.clone(),
            )),
            span: find_span($snippet, &$self.toml_src),
        }
    };
}

impl Validator {
    pub fn new(befaring: &Befaring, toml_src: &str, file_name: &str) -> Self {
        Self {
            befaring: befaring.clone(),
            toml_src: toml_src.into(),
            file_name: file_name.into(),
        }
    }

    pub fn validate(&mut self) -> miette::Result<(IR, EnvSetup), ValidationError> {
        let tests = self.validate_tests()?;
        let setup = self.validate_setup()?;

        Ok((tests, setup))
    }

    fn validate_tests(&self) -> Result<IR, ValidationError> {
        let before_each = self.create_before_each(&self.befaring.before_each)?;

        let test_groups = self
            .befaring
            .test_groups
            .iter()
            .map(|group| {
                let before_each = self.create_before_each(&group.before_each)?;

                let file_name = self.file_name.clone();
                let toml_src = self.toml_src.clone();

                let tests: Vec<ValidatedTests> = group
                    .tests
                    .iter()
                    .map(|test| {
                        self.create_test(
                            &test,
                            file_name.as_ref(),
                            toml_src.as_ref(),
                            &self.befaring.setup.base_url,
                        )
                    })
                    .collect::<Result<Vec<_>, ValidationError>>()?;

                Ok(TestGroups {
                    name: "name".into(),
                    before_each,
                    tests,
                })
            })
            .collect::<Result<Vec<_>, ValidationError>>()?;

        Ok(IR {
            before_each,
            tests: test_groups,
        })
    }

    fn validate_setup(&self) -> Result<EnvSetup, ValidationError> {
        Ok(EnvSetup {
            base_url: self.befaring.setup.base_url.clone(),
            command: self.befaring.setup.command.clone(),
            args: self.befaring.setup.args.clone(),
            ready_when: self.befaring.setup.ready_when.clone(),
            db_type: self.befaring.db.db_type.clone(),
            migration_dir: Some(self.befaring.db.migration_dir.clone()),
            db_port: self.befaring.db.port,
            database_url_env: self
                .befaring
                .setup
                .database_url_env
                .clone()
                .unwrap_or("DATABASE_URL".into()),
        })
    }

    fn create_before_each(
        &self,
        hook: &Option<Hook>,
    ) -> Result<Option<BeforeEach>, ValidationError> {
        if let Some(hook) = hook {
            Ok(Some(BeforeEach {
                reset_db: Some(hook.reset.unwrap_or(false)),
                sql: Some(hook.run_sql.clone().unwrap_or_default()),
            }))
        } else {
            Ok(None)
        }
    }

    fn create_test(
        &self,
        test: &parser::Test,
        file_name: &str,
        toml_src: &str,
        base_url: &str,
    ) -> Result<ValidatedTests, ValidationError> {
        let method = parse_method(&test.method.to_uppercase()).map_err(|e| {
            validation_err!(format!("{} - method", test.name), e, self, &test.method)
        })?;

        let url = parse_url(&base_url, &test.url).map_err(|e| match e {
            ParseUrlError::SetupUrlEndsWithSlash => {
                validation_err!("setup.base_url", BASE_URL_ENDS_WITH, self, &base_url)
            }

            ParseUrlError::PathUrlMissingSlash => validation_err!(
                format!("{}/url", test.name),
                PATH_URL_MISSING_SLASH,
                self,
                &test.url
            ),
            ParseUrlError::ParseIntoUrlFailed(parse_error) => validation_err!(
                format!("{}/url", &base_url),
                parse_error.to_string(),
                self,
                &base_url
            ),
        })?;

        let body = test.body.clone();
        let name = test.name.clone();

        let assertions = parser_assertion::parse_assertions(
            &test.assert_status,
            &test.assert_headers,
            Some((file_name, toml_src)),
        )?;

        Ok(ValidatedTests {
            name,
            body,
            method,
            url,
            assertions,
        })
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

fn find_span(needle: &str, toml_src: &str) -> Option<SourceSpan> {
    let pattern = format!("\"{}\"", needle);
    toml_src
        .find(&pattern)
        .map(|start| SourceSpan::new(start.into(), needle.len()))
}
