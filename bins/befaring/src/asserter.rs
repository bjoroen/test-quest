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
            // ❌ SQL assertion mismatch
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

            _ => todo!(),
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
        }
    }
}

pub trait Assert {
    fn assert(&self) -> Arc<[AssertResult]>;
}

impl Assert for RunnerResult {
    fn assert(&self) -> Arc<[AssertResult]> {
        let Ok(request) = &self.request else { todo!() };

        Arc::from(
            self.assertions
                .iter()
                .map(|a| {
                    let result = match a {
                        Assertion::Status(expected_status) => {
                            assert_status(expected_status, request.status())
                        }
                        Assertion::Headers(expected_headermap) => {
                            assert_header(expected_headermap, request.headers())
                        }
                        Assertion::Sql { expect, got, .. } => assert_sql(expect, got.as_ref()),
                    };

                    AssertResult {
                        status: result,
                        expected: a.clone(),
                        actual: match a {
                            Assertion::Status(_) => Actual::Status(request.status()),
                            Assertion::Headers(_) => Actual::Header(request.headers().clone()),
                            Assertion::Sql { got, .. } => {
                                if let Some(g) = got {
                                    Actual::Sql(g.clone())
                                } else {
                                    Actual::Sql("".into())
                                }
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
