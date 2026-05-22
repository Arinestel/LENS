use std::fmt;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

const OLLAMA_HEALTH_REQUEST: &str =
    "GET /api/tags HTTP/1.1\r\nHost: 127.0.0.1:11434\r\nConnection: close\r\n\r\n";
const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const IO_TIMEOUT: Duration = Duration::from_millis(700);
const STARTUP_CHECK_DELAY: Duration = Duration::from_millis(500);
const STARTUP_CHECK_ATTEMPTS: usize = 8;
const OLLAMA_NOT_RUNNING_MESSAGE: &str = "Ollama server is not running";

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerOneStatus {
    Running,
    NotRunning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerOneModelStatus {
    Available,
    Unavailable,
}

#[derive(Debug, Clone)]
pub struct ServerControlError {
    message: String,
}

impl ServerControlError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ServerControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ServerControlError {}

pub fn check_server_one_status() -> Result<ServerOneStatus, ServerControlError> {
    let response = match read_ollama_tags_response() {
        Ok(response) => response,
        Err(error) if error.to_string() == OLLAMA_NOT_RUNNING_MESSAGE => {
            return Ok(ServerOneStatus::NotRunning);
        }
        Err(error) => return Err(error),
    };
    let status_code = http_status_code(&response)?;

    if (200..300).contains(&status_code) {
        Ok(ServerOneStatus::Running)
    } else {
        Err(ServerControlError::new(format!(
            "Ollama status check returned HTTP {status_code}"
        )))
    }
}

pub fn check_server_one_model_status(
    model_name: &str,
) -> Result<ServerOneModelStatus, ServerControlError> {
    let model_name = model_name.trim();
    if model_name.is_empty() {
        return Ok(ServerOneModelStatus::Unavailable);
    }

    let response = read_ollama_tags_response()?;
    let status_code = http_status_code(&response)?;
    if !(200..300).contains(&status_code) {
        return Err(ServerControlError::new(format!(
            "Ollama model status check returned HTTP {status_code}"
        )));
    }

    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_default();
    let tags_json = serde_json::from_str::<serde_json::Value>(body).map_err(|error| {
        ServerControlError::new(format!("failed to parse Ollama model list: {error}"))
    })?;
    let is_available = tags_json
        .get("models")
        .and_then(|models| models.as_array())
        .map(|models| {
            models.iter().any(|model| {
                model
                    .get("name")
                    .and_then(|name| name.as_str())
                    .map(|name| name == model_name)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    if is_available {
        Ok(ServerOneModelStatus::Available)
    } else {
        Ok(ServerOneModelStatus::Unavailable)
    }
}

fn read_ollama_tags_response() -> Result<String, ServerControlError> {
    let ollama_address = SocketAddr::from(([127, 0, 0, 1], 11434));
    let mut stream = match TcpStream::connect_timeout(&ollama_address, CONNECT_TIMEOUT) {
        Ok(stream) => stream,
        Err(error) if is_not_running_connection_error(&error) => {
            return Err(ServerControlError::new(OLLAMA_NOT_RUNNING_MESSAGE));
        }
        Err(error) => {
            return Err(ServerControlError::new(format!(
                "failed to check Ollama server status: {error}"
            )));
        }
    };

    stream.set_read_timeout(Some(IO_TIMEOUT)).map_err(|error| {
        ServerControlError::new(format!("failed to set Ollama read timeout: {error}"))
    })?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|error| {
            ServerControlError::new(format!("failed to set Ollama write timeout: {error}"))
        })?;
    stream
        .write_all(OLLAMA_HEALTH_REQUEST.as_bytes())
        .map_err(|error| {
            ServerControlError::new(format!("failed to send Ollama status request: {error}"))
        })?;

    let mut response = String::new();
    stream.read_to_string(&mut response).map_err(|error| {
        ServerControlError::new(format!("failed to read Ollama status response: {error}"))
    })?;

    Ok(response)
}

fn http_status_code(response: &str) -> Result<u16, ServerControlError> {
    let status_line = response
        .lines()
        .next()
        .ok_or_else(|| ServerControlError::new("empty Ollama status response"))?;
    status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| {
            ServerControlError::new(format!("invalid Ollama status response: {status_line}"))
        })
}

pub fn start_server_one_if_needed() -> Result<ServerOneStatus, ServerControlError> {
    if check_server_one_status()? == ServerOneStatus::Running {
        return Ok(ServerOneStatus::Running);
    }

    let mut command = Command::new("ollama");
    command
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command.spawn().map_err(|error| {
        ServerControlError::new(format!("failed to start Ollama server: {error}"))
    })?;

    let mut last_error = None;
    for _ in 0..STARTUP_CHECK_ATTEMPTS {
        thread::sleep(STARTUP_CHECK_DELAY);
        match check_server_one_status() {
            Ok(ServerOneStatus::Running) => return Ok(ServerOneStatus::Running),
            Ok(ServerOneStatus::NotRunning) => {}
            Err(error) => last_error = Some(error),
        }
    }

    if let Some(error) = last_error {
        return Err(ServerControlError::new(format!(
            "Ollama server start was requested, but status check failed: {error}"
        )));
    }

    Err(ServerControlError::new(
        "Ollama server start was requested, but status check still reports not running",
    ))
}

fn is_not_running_connection_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::TimedOut
    )
}
