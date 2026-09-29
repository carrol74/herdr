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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientTheme {
    pub name: String,
    pub colors: std::collections::HashMap<String, Option<String>>,
}

pub fn client_theme(path: &Path) -> Result<Option<ClientTheme>, ApiError> {
    let result = request(path, "client.theme.get", serde_json::json!({}))?;
    Ok(serde_json::from_value(
        result.get("theme").cloned().unwrap_or_default(),
    )?)
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
    Server(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Closed => write!(formatter, "connection closed"),
            Self::Server(error) => write!(formatter, "{error}"),
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

pub fn active_session() -> String {
    if std::env::var_os("HERDR_SOCKET_PATH").is_some() {
        return "custom".to_string();
    }
    std::env::var("HERDR_SESSION")
        .ok()
        .filter(|session| session != "default" && valid_session_name(session))
        .unwrap_or_else(|| "default".to_string())
}

pub fn socket_path_for_session(session: &str) -> Option<PathBuf> {
    if session == "custom" {
        return std::env::var("HERDR_SOCKET_PATH").ok().map(PathBuf::from);
    }
    if !valid_session_name(session) {
        return None;
    }
    let base = config_directory();
    Some(if session == "default" {
        base.join("herdr.sock")
    } else {
        base.join("sessions").join(session).join("herdr.sock")
    })
}

pub fn available_sessions(active: &str) -> Vec<String> {
    if std::env::var_os("HERDR_SOCKET_PATH").is_some() {
        return vec!["custom".to_string()];
    }
    let mut sessions = vec!["default".to_string()];
    if let Ok(entries) = std::fs::read_dir(config_directory().join("sessions")) {
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if valid_session_name(&name) && entry.path().join("herdr.sock").exists() {
                sessions.push(name);
            }
        }
    }
    if valid_session_name(active)
        && !sessions.iter().any(|session| session == active)
        && socket_path_for_session(active)
            .map(|path| path.exists())
            .unwrap_or(false)
    {
        sessions.push(active.to_string());
    }
    sessions.sort_unstable();
    sessions.dedup();
    sessions
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
    let result = request(path, "agent.list", serde_json::json!({}))?;
    Ok(serde_json::from_value::<ListResult>(result)?.agents)
}

pub fn activate_client(path: &Path) -> Result<(), ApiError> {
    let result = request(path, "client.activate", serde_json::json!({}))?;
    activation_result(&result)
}

fn activation_result(result: &serde_json::Value) -> Result<(), ApiError> {
    if result.get("activated").and_then(serde_json::Value::as_bool) == Some(true) {
        return Ok(());
    }
    let reason = result
        .get("reason")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("failed");
    let message = match reason {
        "no_foreground_client" => "没有可激活的 Herdr 客户端",
        "client_unavailable" => "Herdr 客户端当前不可用",
        "unsupported_terminal" => "当前终端暂不支持自动显示 Herdr 窗口",
        "permission_denied" => "macOS 未允许 Herdr 控制 Ghostty",
        "terminal_not_found" => "找不到承载 Herdr 的 Ghostty 窗口",
        "busy" => "Herdr 正在处理另一次窗口切换",
        "timed_out" => "显示 Herdr 窗口超时",
        _ => "无法显示 Herdr 窗口",
    };
    Err(ApiError::Server(message.to_string()))
}

pub fn request(
    path: &Path,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let request = serde_json::json!({"id": "pet:request", "method": method, "params": params});
    let mut stream = connect(path)?;
    stream.write_all(serde_json::to_string(&request)?.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
        return Err(ApiError::Closed);
    }
    let response = serde_json::from_str::<serde_json::Value>(&line)?;
    if let Some(error) = response.get("error") {
        let message = error
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Herdr request failed");
        return Err(ApiError::Server(message.to_string()));
    }
    response
        .get("result")
        .cloned()
        .ok_or_else(|| ApiError::Server("Herdr response has no result".to_string()))
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

    #[test]
    fn activation_result_requires_confirmed_activation() {
        assert!(activation_result(&serde_json::json!({
            "activated": true,
            "reason": "activated"
        }))
        .is_ok());
        assert_eq!(
            activation_result(&serde_json::json!({
                "activated": false,
                "reason": "permission_denied"
            }))
            .unwrap_err()
            .to_string(),
            "macOS 未允许 Herdr 控制 Ghostty"
        );
    }
}
