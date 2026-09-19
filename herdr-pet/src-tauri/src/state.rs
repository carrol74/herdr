use serde::Serialize;

use crate::api::AgentStatus;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubjectInfo {
    pub pane_id: String,
    pub workspace_id: String,
    pub agent: Option<String>,
    pub title: Option<String>,
    pub status: AgentStatus,
    pub cwd: Option<String>,
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
    pub offline_reason: Option<String>,
}

impl Default for PetState {
    fn default() -> Self {
        Self {
            connection: Connection::Offline,
            mood: Mood::Offline,
            subject: None,
            offline_reason: None,
        }
    }
}
