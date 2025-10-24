use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use thiserror::Error;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::sleep;

#[derive(Debug)]
pub enum OutputSource {
    StdOut,
    StdErr,
}

#[derive(Debug)]
pub struct OutputLine {
    pub source: OutputSource,
    pub line: String,
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("failed to spawn app process: {0}")]
    SpawningAppFailed(std::io::Error),

    #[error("timedout while waiting for app to response on: {0}")]
    Timeout(String),

    #[error("Error while reading messages from stdin: {0}")]
    PipeAccessError(String),
}

pub struct AppProcess {
    pub process: Arc<Mutex<Child>>,
    pub output: Arc<Mutex<Vec<OutputLine>>>,
}

pub async fn from_command(
    command: String,
    args: Option<Vec<String>>,
    database_env: String,
    database_url: String,
    stream_app: bool,
) -> Result<AppProcess, AppError> {
    let output_buffer = Arc::new(Mutex::new(Vec::new()));

    // Clone buffer references for the background tasks
    let stdout_task_buffer = output_buffer.clone();
    let stderr_task_buffer = output_buffer.clone();

    let mut app_process = Command::new(command)
        .args(args.unwrap_or_default())
        .env(database_env, &database_url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(AppError::SpawningAppFailed)?;

    let stdout = app_process
        .stdout
        .take()
        .ok_or_else(|| AppError::PipeAccessError("Could not take stdout handle".to_string()))?;
    let stderr = app_process
        .stderr
        .take()
        .ok_or_else(|| AppError::PipeAccessError("Could not take stderr handle".to_string()))?;

    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let mut buffer = stdout_task_buffer.lock().await;
            if stream_app {
                println!("[ STDOUT ] {line}")
            }

            buffer.push(OutputLine {
                source: OutputSource::StdOut,
                line,
            });
        }
    });

    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let mut buffer = stderr_task_buffer.lock().await;
            if stream_app {
                println!("[ stderr ] {line}")
            }
            buffer.push(OutputLine {
                source: OutputSource::StdErr,
                line,
            });
        }
    });

    Ok(AppProcess {
        process: Arc::new(Mutex::new(app_process)),
        output: output_buffer,
    })
}

pub async fn wait_for_app_ready(base_url: &str, ready_when_url: &str) -> Result<(), AppError> {
    let client = Client::new();
    let mut elapsed = 0;
    let timeout_secs = 15;

    let url = format!("{}{}", base_url, ready_when_url);
    while elapsed < timeout_secs {
        if let Ok(resp) = client.get(&url).send().await
            && resp.status().is_success()
        {
            return Ok(());
        }

        sleep(Duration::from_secs(1)).await;
        elapsed += 1;
    }

    Err(AppError::Timeout(url))
}
