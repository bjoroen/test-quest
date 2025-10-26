use core::fmt;
use std::fmt::Display;
use std::sync::Arc;

use flume::Receiver;
use flume::Sender;
use reqwest::StatusCode;
use reqwest::header::HeaderMap;

use crate::parser::StringOrStrings;
use crate::runner::RunnerResult;
use crate::validator::Assertion;

pub struct Asserter {}

#[derive(Debug, Clone, Eq, PartialEq)]
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
    Sql(Vec<String>),
    Json(serde_json::Value),
    RequestFailed(String),
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
                    console::style("✖").red().bold(),
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
                writeln!(f, "  {}", console::style("Expected rows:").green().bold())?;
                match expect {
                    StringOrStrings::Single(s) => {
                        writeln!(
                            f,
                            "    {}",
                            console::style(format!("{:>2}: {}", 1, s)).green()
                        )?;
                    }
                    StringOrStrings::Multiple(items) => {
                        for (i, row) in items.iter().enumerate() {
                            writeln!(
                                f,
                                "    {}",
                                console::style(format!("{:>2}: {}", i + 1, row)).green()
                            )?;
                        }
                    }
                }

                match got.len() {
                    0 => {
                        writeln!(
                            f,
                            "  {} {}",
                            console::style("Got:").red(),
                            console::style("<no rows returned>").red().bold()
                        )
                    }
                    // 1 => {
                    //     writeln!(
                    //         f,
                    //         "  {} {}",
                    //         console::style("Got row:").red(),
                    //         console::style(&got[0]).red().bold()
                    //     )
                    // }
                    _ => {
                        writeln!(f, "  {}", console::style("Got rows:").red().bold())?;
                        for (i, row) in got.iter().enumerate() {
                            writeln!(
                                f,
                                "    {}",
                                console::style(format!("{:>2}: {}", i + 1, row)).red()
                            )?;
                        }
                        Ok(())
                    }
                }
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
            (TestResult::Fail, _, Actual::RequestFailed(err)) => {
                writeln!(
                    f,
                    "{} {}",
                    console::style("✘").red().bold(),
                    console::style("FAIL!").red().bold(),
                )?;
                writeln!(
                    f,
                    "  {} {}",
                    console::style("Request failed with error:").red(),
                    console::style(err).red().bold()
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
            Assertion::RequestFailed => write!(f, "Request failed"),
        }
    }
}

impl Display for Actual {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Actual::Header(header_map) => {
                let headers: Vec<String> = header_map
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v.to_str().unwrap_or("<invalid utf8>")))
                    .collect();
                write!(f, "Got headers {{{}}}", headers.join(", "))
            }
            Actual::Status(status_code) => write!(f, "Got status {}", status_code),
            Actual::Sql(sqls) => {
                if sqls.len() == 1 {
                    write!(f, "Got response from database: {}", sqls[0])
                } else {
                    write!(f, "Got responses from database: [{}]", sqls.join(", "))
                }
            }
            Actual::Json(value) => write!(f, "Got json: {value}"),
            Actual::RequestFailed(_) => write!(f, "Request failed"),
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
                status: TestResult::Fail,
                expected: Assertion::RequestFailed,
                actual: Actual::RequestFailed(error.to_string()),
            }]);
        }

        let Some(response) = &self.response else {
            return Arc::from([AssertResult {
                status: TestResult::Fail,
                expected: Assertion::RequestFailed,
                actual: Actual::RequestFailed(self.error.clone().unwrap_or_default()),
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
                        Assertion::RequestFailed => todo!(),
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
                                    Actual::Sql(vec![])
                                }
                            }
                            Assertion::Json(_) => {
                                Actual::Json(response.body_json.clone().unwrap_or_default())
                            }
                            Assertion::RequestFailed => todo!(),
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
        output_tx: Sender<(String, String, String, Arc<[AssertResult]>)>,
    ) -> Result<(), ()> {
        while let Ok(msg) = rx.recv_async().await {
            let assert_result = msg.assert();

            let path = msg.url.path();
            let method = msg.method;
            if let Err(error) = output_tx
                .send_async((msg.name, path.into(), method, assert_result))
                .await
            {
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

fn assert_sql(expect: &StringOrStrings, got: Option<&Vec<String>>) -> TestResult {
    match expect {
        StringOrStrings::Single(expected) => {
            let Some(got) = got else {
                return TestResult::Fail;
            };

            if expected.is_empty() && got.is_empty() {
                return TestResult::Pass;
            }

            if got.len() != 1 {
                return TestResult::Fail;
            }

            if got[0] != *expected {
                return TestResult::Fail;
            }
        }

        StringOrStrings::Multiple(expected_items) => {
            let Some(got) = got else {
                return TestResult::Fail;
            };

            if got.len() != expected_items.len() {
                return TestResult::Fail;
            }

            for (expected, actual) in expected_items.iter().zip(got.iter()) {
                if expected != actual {
                    return TestResult::Fail;
                }
            }
        }
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

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use reqwest::StatusCode;
    use reqwest::header::HOST;
    use reqwest::header::HeaderMap;
    use reqwest::header::LOCATION;
    use url::Url;

    use crate::asserter::AssertResult;
    use crate::asserter::Asserter;
    use crate::asserter::TestResult;
    use crate::runner::CapturedResponse;
    use crate::runner::RunnerResult;
    use crate::validator::Assertion;

    #[test]
    fn assert_status_test() {
        // TODO: Write tests
    }
    #[test]
    fn assert_headers() {
        // TODO: Write tests
    }
    #[test]
    fn assert_json() {
        // TODO: Write tests
    }

    #[test]
    fn assert_db_state() {
        // TODO: Write tests
    }

    #[tokio::test]
    async fn test_full() {
        let (runner_tx, asserter_rx) = flume::unbounded::<RunnerResult>();
        let (asserter_tx, outputter_rx) =
            flume::unbounded::<(String, String, String, Arc<[AssertResult]>)>();

        tokio::spawn(async move {
            Asserter::run(asserter_rx, asserter_tx).await.unwrap();
        });

        let mut header_map = HeaderMap::new();

        header_map.insert(HOST, "world".parse().unwrap());
        header_map.insert(LOCATION, "this-is-a-location".parse().unwrap());

        let json_data = r#"
        {
            "name": "John Doe",
            "age": 43,
            "phones": [
                "+44 1234567",
                "+44 2345678"
            ]
        }"#;

        runner_tx
            .send_async(RunnerResult {
                name: "this-is-a-name".into(),
                method: "GET".into(),
                url: Url::parse("http://test.com/some-path").unwrap(),
                response: Some(CapturedResponse {
                    status: StatusCode::OK,
                    headers: header_map.clone(),
                    body_text: None,
                    body_json: Some(serde_json::from_str(json_data).unwrap()),
                }),
                error: None,
                assertions: vec![
                    Assertion::Status(200),
                    Assertion::Headers(header_map),
                    Assertion::Json(serde_json::from_str(json_data).unwrap()),
                ],
            })
            .await
            .unwrap();

        let Ok((name, path, method, result)) = outputter_rx.recv_async().await else {
            todo!()
        };
        assert_eq!(name, "this-is-a-name");
        assert_eq!(path, "/some-path");
        assert_eq!(method, "GET");

        for res in result.iter() {
            assert_eq!(res.status, TestResult::Pass);
        }
    }
}
