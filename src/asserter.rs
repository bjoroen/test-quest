use flume::Receiver;
use flume::Sender;
use reqwest::StatusCode;
use thiserror::Error;
use tokio::task;

use crate::Stage;
use crate::runner::RunnerResult;
use crate::validator::Assertions;

pub struct Asserter {}
pub struct Output {}
#[derive(Debug)]
pub struct OutputError(Vec<Asserts>);

#[derive(Debug)]
pub struct Asserts {
    name: String,
    results: Vec<AssertionResult>,
}

#[derive(Debug, Error)]
pub enum AssertionError {
    #[error("Request error")]
    RequestError,
}

#[derive(Debug)]
pub enum AssertionResult {
    Status(String),
    Header(String),
}

pub trait Assert {
    fn assert(&self, tx: &Sender<(i32, Stage)>) -> Result<AssertionResult, AssertionError>;
}

impl Assert for RunnerResult {
    fn assert(&self, tx: &Sender<(i32, Stage)>) -> Result<AssertionResult, AssertionError> {
        let Ok(request) = &self.request else {
            return Err(AssertionError::RequestError);
        };

        self.assertions.iter().map(|a| {
            todo!();
            todo!()
        });

        todo!()
    }
}

impl Asserter {
    pub async fn run(
        rx: Receiver<RunnerResult>,
        output_tx: Sender<(i32, Stage)>,
    ) -> Result<(), OutputError> {
        let rx_task = task::spawn(async move {
            while let Ok(msg) = rx.recv_async().await {
                let results = msg.assert(&output_tx);
            }
        })
        .await;

        Ok(())
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
