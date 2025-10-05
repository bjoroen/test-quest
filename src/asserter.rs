use reqwest::StatusCode;

use crate::runner::RunnerResult;
use crate::validator::Assertions;

pub struct Asserter {}
pub struct Output {}
pub struct OutputError(Vec<Asserts>);

#[derive(Debug)]
pub struct Asserts {
    name: String,
    results: Vec<AssertionResult>,
}

#[derive(Debug)]
pub enum AssertionResult {
    Status(String),
    Header(String),
}

impl Asserter {
    pub fn run(runner_results: Vec<RunnerResult>) -> Result<Vec<Asserts>, OutputError> {
        /// This whole function with asssers, assertresuls, vecs here and there
        /// is super confusing!!?><?>
        let mut some_name = vec![];

        for result in runner_results {
            let mut assert_result = vec![];
            let Ok(r) = result.request else { todo!() };

            for asss in result.assertions {
                let ass = match asss {
                    Assertions::Status(s) => assert_status(&s, r.status()),
                    Assertions::Headers(hash_map) => assert_header(&hash_map, r.headers()),
                };

                assert_result.push(ass)
            }

            some_name.push(Asserts {
                name: result.name,
                results: assert_result,
            })
        }

        Ok(some_name)
    }
}

fn assert_header(
    hash_map: &std::collections::HashMap<String, String>,
    headers: &reqwest::header::HeaderMap,
) -> AssertionResult {
    AssertionResult::Header("Passed".into())
}

fn assert_status(s: &i32, status: reqwest::StatusCode) -> AssertionResult {
    let inncomming_status_code = match StatusCode::from_u16(*s as u16) {
        Ok(status) => status,
        Err(_) => todo!(),
    };

    if inncomming_status_code != status {
        return AssertionResult::Status("Failed".into());
    }

    AssertionResult::Status("Passed".into())
}
