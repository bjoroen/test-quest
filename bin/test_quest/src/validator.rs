use std::path::PathBuf;
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
use crate::parser::Global;
use crate::parser::Hook;
use crate::parser::ImageRef;
use crate::parser::TestQuest;

// Error messages for parsing URLs
const BASE_URL_ENDS_WITH: &str =
    "The base URL from setup can’t end with a /, and each URL in test must start with one";
const PATH_URL_MISSING_SLASH: &str =
    "The URL field in a test is required to begin with a leading /.";

pub struct Validator {
    test_quest: TestQuest,
    toml_src: String,
    file_name: String,
}

#[derive(Debug, Clone)]
pub enum Assertion {
    Status(i32),
    Headers(HeaderMap),
    Sql {
        query: String,
        expect: String,
        got: Option<String>,
    },
    Json(serde_json::Value),
    RequestFailed,
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
    pub init_sql: Option<PathBuf>,
    pub image_ref: Option<ImageRef>,
}

pub struct IR {
    pub before_each_group: Option<BeforeEach>,
    pub tests: Vec<TestGroups>,
}

pub struct TestGroups {
    pub name: String,
    pub before_group: Option<BeforeEach>,
    pub before_each_test: Option<BeforeEach>,
    pub tests: Vec<ValidatedTests>,
}

pub struct BeforeEach {
    pub reset_db: Option<bool>,
    pub sql: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct ValidatedTests {
    pub before_run: Option<Vec<String>>,
    pub name: String,
    pub method: Method,
    pub url: Url,
    pub headers: HeaderMap,
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
    pub fn new(test_quest: &TestQuest, toml_src: &str, file_name: &str) -> Self {
        Self {
            test_quest: test_quest.clone(),
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
        let before_each_group = self.create_before_each(&self.test_quest.before_each_group)?;

        let test_groups = self
            .test_quest
            .test_groups
            .iter()
            .map(|group| {
                let before_each_test = self.create_before_each(&group.before_each_test)?;
                let before_group = self.create_before_each(&group.before_group)?;
                let name = group.name.clone();

                let file_name = self.file_name.clone();
                let toml_src = self.toml_src.clone();

                let tests: Vec<ValidatedTests> = group
                    .tests
                    .iter()
                    .map(|test| {
                        self.create_test(
                            test,
                            file_name.as_ref(),
                            toml_src.as_ref(),
                            &self.test_quest.setup.base_url,
                            &self.test_quest.global,
                        )
                    })
                    .collect::<Result<Vec<_>, ValidationError>>()?;

                Ok(TestGroups {
                    name,
                    before_each_test,
                    before_group,
                    tests,
                })
            })
            .collect::<Result<Vec<_>, ValidationError>>()?;

        Ok(IR {
            before_each_group,
            tests: test_groups,
        })
    }

    fn validate_setup(&self) -> Result<EnvSetup, ValidationError> {
        let path = self.test_quest.db.init_sql.as_ref().map(PathBuf::from);

        Ok(EnvSetup {
            base_url: self.test_quest.setup.base_url.clone(),
            command: self.test_quest.setup.command.clone(),
            args: self.test_quest.setup.args.clone(),
            ready_when: self.test_quest.setup.ready_when.clone(),
            db_type: self.test_quest.db.db_type.clone(),
            migration_dir: Some(self.test_quest.db.migration_dir.clone()),
            db_port: self.test_quest.db.port,
            init_sql: path,
            image_ref: self.test_quest.db.image_ref.clone(),
            database_url_env: self
                .test_quest
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
        global: &Global,
    ) -> Result<ValidatedTests, ValidationError> {
        let method = parse_method(&test.method.to_uppercase()).map_err(|e| {
            validation_err!(format!("{} - method", test.name), e, self, &test.method)
        })?;

        let url = parse_url(base_url, &test.url, test.query.as_deref()).map_err(|e| match e {
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
        let before_run = test.before_run.clone();

        // Start with the global headers if defined, and add them to the request's
        // HeaderMap. Then, merge the headers from the individual test. If a
        // header exists in both the global and test headers, the test header
        // takes precedence.
        let mut headers = if let Some(global_value) = &global.headers {
            parser_assertion::parse_header_map(
                global_value,
                Some(&(file_name.to_string(), toml_src.to_string())),
            )?
        } else {
            HeaderMap::new()
        };

        if let Some(header_value) = &test.headers {
            let test_headers = parser_assertion::parse_header_map(
                header_value,
                Some(&(file_name.to_string(), toml_src.to_string())),
            )?;

            for (key, value) in test_headers {
                if let Some(key) = key {
                    headers.insert(key, value);
                }
            }
        }

        let assertions = parser_assertion::parse_assertions(
            &test.assert_status,
            &test.assert_headers,
            &test.assert_sql,
            &test.assert_json,
            Some((file_name, toml_src)),
        )?;

        Ok(ValidatedTests {
            before_run,
            name,
            body,
            method,
            headers,
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
fn parse_url(base_url: &str, path_url: &str, query: Option<&str>) -> Result<Url, ParseUrlError> {
    if base_url.ends_with("/") {
        return Err(ParseUrlError::SetupUrlEndsWithSlash);
    }

    if !path_url.starts_with("/") {
        return Err(ParseUrlError::PathUrlMissingSlash);
    }

    let url_string = query.map_or_else(
        || format!("{base_url}{path_url}"),
        |query| format!("{base_url}{path_url}{query}"),
    );

    let url =
        reqwest::Url::parse(url_string.as_str()).map_err(ParseUrlError::ParseIntoUrlFailed)?;

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
