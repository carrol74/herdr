use serde::Serialize;

use crate::api::AgentStatus;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubjectInfo {
    pub pane_id: String,
    pub agent: Option<String>,
    pub title: Option<String>,
    pub status: AgentStatus,
    pub cwd: Option<String>,
    pub focused: bool,
    pub status_since_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub name: String,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Connection {
    Online,
    Offline,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mood {
    Sleeping,
    Working,
    Attention,
    Celebrate,
    Neutral,
    Offline,
}

impl Mood {
    pub fn from_status(status: AgentStatus) -> Self {
        match status {
            AgentStatus::Idle => Self::Sleeping,
            AgentStatus::Working => Self::Working,
            AgentStatus::Blocked => Self::Attention,
            AgentStatus::Done => Self::Celebrate,
            AgentStatus::Unknown => Self::Neutral,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetState {
    pub connection: Connection,
    pub mood: Mood,
    pub subject: Option<SubjectInfo>,
    pub agents: Vec<SubjectInfo>,
    pub blocked_count: usize,
    pub session: String,
    pub sessions: Vec<SessionInfo>,
    pub offline_reason: Option<String>,
    pub theme: Option<crate::api::ClientTheme>,
}

impl Default for PetState {
    fn default() -> Self {
        Self {
            connection: Connection::Offline,
            mood: Mood::Offline,
            subject: None,
            agents: Vec::new(),
            blocked_count: 0,
            session: "default".to_string(),
            sessions: Vec::new(),
            offline_reason: None,
            theme: None,
        }
    }
}
