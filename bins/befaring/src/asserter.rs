use core::fmt;
use std::fmt::Display;
use std::sync::Arc;

use flume::Receiver;
use flume::Sender;
use reqwest::StatusCode;
use reqwest::header::HeaderMap;

use crate::runner::RunnerResult;
use crate::validator::Assertion;

pub struct Asserter {}

#[derive(Debug, Clone)]
pub enum TestResult {
    Pass,
    Fail,
}

#[derive(Debug, Clone)]
pub struct AssertResult {
    pub status: TestResult,
    pub expected: Assertion,
    pub actual: Actual,
}

#[derive(Debug, Clone)]
pub enum Actual {
    Header(HeaderMap),
    Status(reqwest::StatusCode),
    Sql(String),
    Json(serde_json::Value),
}

impl Display for AssertResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.status, &self.expected, &self.actual) {
            (TestResult::Pass, _, actual) => {
                write!(
                    f,
                    "{} {} {}",
                    console::style("✔").green().bold(),
                    console::style("PASS!").green().bold(),
                    actual
                )
            }

            (TestResult::Fail, Assertion::Status(exp), Actual::Status(act)) => {
                write!(
                    f,
                    "{} {}\n  Expected: {}\n  Actual:   {}",
                    console::style("✘").red().bold(),
                    console::style("FAIL!").red().bold(),
                    console::style(format!("Expected status {}", exp)).green(),
                    console::style(format!("Got status {}", act)).red(),
                )
            }

            (
                TestResult::Fail,
                Assertion::Headers(expected_headers),
                Actual::Header(actual_headers),
            ) => {
                writeln!(
                    f,
                    "{} {}",
                    console::style("✘").red().bold(),
                    console::style("FAIL!").red().bold(),
                )?;
                writeln!(f, "  {}", console::style("Expected headers:").green())?;
                print_headers(f, expected_headers)?;
                writeln!(f, "  {}", console::style("Actual headers:").red())?;
                print_headers(f, actual_headers)
            }
            (TestResult::Fail, Assertion::Sql { query, expect, .. }, Actual::Sql(got)) => {
                writeln!(
                    f,
                    "{} {}",
                    console::style("✘").red().bold(),
                    console::style("FAIL!").red().bold(),
                )?;
                writeln!(f, "  {}", console::style("SQL query:").yellow().bold())?;
                writeln!(f, "    {}", console::style(query).dim())?;
                writeln!(
                    f,
                    "  {} {}",
                    console::style("Expected:").green(),
                    console::style(expect).green().bold()
                )?;
                writeln!(
                    f,
                    "  {} {}",
                    console::style("Got:").red(),
                    console::style(got).red().bold()
                )
            }

            (TestResult::Fail, Assertion::Json(expected_json), Actual::Json(actual_json)) => {
                writeln!(
                    f,
                    "{} {}",
                    console::style("✘").red().bold(),
                    console::style("FAIL!").red().bold(),
                )?;
                writeln!(f, "  {}", console::style("Expected JSON:").green())?;
                writeln!(
                    f,
                    "{}",
                    console::style(serde_json::to_string_pretty(expected_json).unwrap_or_default())
                        .green()
                )?;
                writeln!(f, "  {}", console::style("Actual JSON:").red())?;
                writeln!(
                    f,
                    "{}",
                    console::style(serde_json::to_string_pretty(actual_json).unwrap_or_default())
                        .red()
                )
            }

            _ => {
                writeln!(
                    f,
                    "{} {} (unhandled combination)",
                    console::style("⚠").yellow(),
                    console::style("UNKNOWN RESULT").yellow().bold()
                )
            }
        }
    }
}

fn print_headers(f: &mut fmt::Formatter<'_>, headers: &HeaderMap) -> fmt::Result {
    for (k, v) in headers.iter() {
        let value = v.to_str().unwrap_or("<invalid utf8>");
        writeln!(
            f,
            "    {}: {}",
            console::style(k.as_str()).yellow().bold(),
            console::style(value)
        )?;
    }
    Ok(())
}

impl Display for Assertion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Assertion::Status(_) => write!(f, "Status test"),
            Assertion::Headers(_) => {
                write!(f, "Header test")
            }
            Assertion::Sql { .. } => write!(f, "SQL test"),
            Assertion::Json(..) => write!(f, "JSON test"),
        }
    }
}

impl Display for Actual {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Actual::Header(header_map) => {
                // Convert headers to a readable string
                let headers: Vec<String> = header_map
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v.to_str().unwrap_or("<invalid utf8>")))
                    .collect();
                write!(f, "Got headers {{{}}}", headers.join(", "))
            }
            Actual::Status(status_code) => write!(f, "Got status {}", status_code),
            Actual::Sql(s) => write!(f, "Got response from database: {s}"),
            Actual::Json(value) => write!(f, "Got json: {value}"),
        }
    }
}

pub trait Assert {
    fn assert(&self) -> Arc<[AssertResult]>;
}

impl Assert for RunnerResult {
    fn assert(&self) -> Arc<[AssertResult]> {
        if let Some(error) = &self.error {
            return Arc::from([AssertResult {
                status: todo!(),
                expected: todo!(),
                actual: todo!(),
            }]);
        }

        let Some(response) = &self.response else {
            return Arc::from([AssertResult {
                status: todo!(),
                expected: todo!(),
                actual: todo!(),
            }]);
        };

        Arc::from(
            self.assertions
                .iter()
                .map(|a| {
                    let result = match a {
                        Assertion::Status(expected_status) => {
                            assert_status(expected_status, response.status)
                        }
                        Assertion::Headers(expected_headermap) => {
                            assert_header(expected_headermap, &response.headers)
                        }
                        Assertion::Sql { expect, got, .. } => assert_sql(expect, got.as_ref()),
                        Assertion::Json(expected_json) => {
                            assert_json(expected_json, response.body_json.as_ref())
                        }
                    };

                    AssertResult {
                        status: result,
                        expected: a.clone(),
                        actual: match a {
                            Assertion::Status(_) => Actual::Status(response.status),
                            Assertion::Headers(_) => Actual::Header(response.headers.clone()),
                            Assertion::Sql { got, .. } => {
                                if let Some(g) = got {
                                    Actual::Sql(g.clone())
                                } else {
                                    Actual::Sql("".into())
                                }
                            }
                            Assertion::Json(_) => {
                                Actual::Json(response.body_json.clone().unwrap_or_default())
                            }
                        },
                    }
                })
                .collect::<Vec<AssertResult>>(),
        )
    }
}

impl Asserter {
    pub async fn run(
        rx: Receiver<RunnerResult>,
        output_tx: Sender<(String, Arc<[AssertResult]>)>,
    ) -> Result<(), ()> {
        while let Ok(msg) = rx.recv_async().await {
            let assert_result = msg.assert();

            if let Err(error) = output_tx.send_async((msg.name, assert_result)).await {
                todo!("{error}")
            };
        }

        Ok(())
    }
}

fn assert_json(expected: &serde_json::Value, got: Option<&serde_json::Value>) -> TestResult {
    match got {
        Some(got) => {
            if got == expected {
                TestResult::Pass
            } else {
                TestResult::Fail
            }
        }
        None => TestResult::Pass,
    }
}

fn assert_sql(expect: &str, got: Option<&String>) -> TestResult {
    let Some(got) = got else {
        return TestResult::Fail;
    };

    if got != expect {
        return TestResult::Fail;
    }

    TestResult::Pass
}

fn assert_header(expected: &HeaderMap, actual: &HeaderMap) -> TestResult {
    for (key, value_a) in expected {
        let Some(value_b) = actual.get(key) else {
            continue;
        };
        if value_a.as_bytes() != value_b.as_bytes() {
            return TestResult::Fail;
        }
    }

    TestResult::Pass
}

fn assert_status(s: &i32, status: reqwest::StatusCode) -> TestResult {
    let inncomming_status_code = match StatusCode::from_u16(*s as u16) {
        Ok(status) => status,
        Err(_) => return TestResult::Fail,
    };

    if inncomming_status_code != status {
        return TestResult::Fail;
    }

    TestResult::Pass
}
