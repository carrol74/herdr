use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Working,
    Blocked,
    Done,
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentInfo {
    pub pane_id: String,
    pub workspace_id: String,
    pub agent: Option<String>,
    pub display_agent: Option<String>,
    pub agent_status: AgentStatus,
    #[serde(default)]
    pub focused: bool,
    #[serde(default)]
    pub terminal_title: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    result: ListResult,
}

#[derive(Debug, Deserialize)]
struct ListResult {
    agents: Vec<AgentInfo>,
}

#[derive(Debug)]
pub enum ApiError {
    Io(io::Error),
    Json(serde_json::Error),
    Closed,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Closed => write!(formatter, "connection closed"),
        }
    }
}

impl From<io::Error> for ApiError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

fn app_directory() -> &'static str {
    if cfg!(debug_assertions) {
        "herdr-dev"
    } else {
        "herdr"
    }
}

fn config_directory() -> PathBuf {
    if let Ok(directory) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(directory).join(app_directory());
    }
    #[cfg(windows)]
    if let Ok(directory) = std::env::var("APPDATA") {
        return PathBuf::from(directory).join(app_directory());
    }
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".config")
        .join(app_directory())
}

pub fn socket_path() -> PathBuf {
    if let Ok(path) = std::env::var("HERDR_SOCKET_PATH") {
        return PathBuf::from(path);
    }
    let base = config_directory();
    match std::env::var("HERDR_SESSION") {
        Ok(session) if session != "default" && valid_session_name(&session) => {
            base.join("sessions").join(session).join("herdr.sock")
        }
        _ => base.join("herdr.sock"),
    }
}

fn valid_session_name(session: &str) -> bool {
    !session.is_empty()
        && session.len() <= 64
        && session != "."
        && session != ".."
        && session
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

pub fn connect(path: &Path) -> io::Result<interprocess::local_socket::Stream> {
    #[cfg(unix)]
    {
        use interprocess::local_socket::{prelude::*, GenericFilePath};
        interprocess::local_socket::Stream::connect(path.to_fs_name::<GenericFilePath>()?)
    }
    #[cfg(windows)]
    {
        use interprocess::local_socket::{prelude::*, GenericNamespaced};
        let name = path.to_string_lossy().to_string();
        interprocess::local_socket::Stream::connect(name.to_ns_name::<GenericNamespaced>()?)
    }
}

pub fn agent_list(path: &Path) -> Result<Vec<AgentInfo>, ApiError> {
    let request = serde_json::json!({
        "id": "pet:list",
        "method": "agent.list",
        "params": {},
    });
    let mut stream = connect(path)?;
    stream.write_all(serde_json::to_string(&request)?.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
        return Err(ApiError::Closed);
    }
    Ok(serde_json::from_str::<ListResponse>(&line)?.result.agents)
}

#[derive(Debug, Deserialize)]
pub struct StatusEvent {
    pub pane_id: String,
    pub agent_status: Option<AgentStatus>,
    #[serde(default)]
    pub title: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal_session_names() {
        assert!(valid_session_name("work-1"));
        assert!(!valid_session_name("../work"));
    }
}
