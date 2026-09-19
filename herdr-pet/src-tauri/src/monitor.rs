use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use interprocess::local_socket::traits::Stream as _;
use tauri::{AppHandle, Emitter};

use crate::api::{self, AgentInfo, AgentStatus};
use crate::state::{Connection, Mood, PetState, SubjectInfo};

const RECONCILE_INTERVAL: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(200);

pub fn start(app: AppHandle, state: Arc<Mutex<PetState>>) {
    std::thread::spawn(move || run_loop(app, state));
}

struct Subscription {
    stream: interprocess::local_socket::Stream,
    pane_id: String,
    buffer: Vec<u8>,
}

impl Subscription {
    fn connect(path: &std::path::Path, pane_id: &str) -> Result<Self, api::ApiError> {
        let request = serde_json::json!({
            "id": "pet:subscribe",
            "method": "events.subscribe",
            "params": {
                "subscriptions": [
                    {"type": "pane.agent_status_changed", "pane_id": pane_id}
                ]
            },
        });
        let mut stream = api::connect(path)?;
        stream.set_nonblocking(true).map_err(api::ApiError::Io)?;
        stream.write_all(serde_json::to_string(&request)?.as_bytes())?;
        stream.write_all(b"\n")?;
        stream.flush()?;
        Ok(Self {
            stream,
            pane_id: pane_id.to_string(),
            buffer: Vec::new(),
        })
    }

    fn drain(&mut self, state: &Arc<Mutex<PetState>>) -> Result<bool, api::ApiError> {
        let mut changed = false;
        let mut chunk = [0_u8; 4096];
        loop {
            match self.stream.read(&mut chunk) {
                Ok(0) => return Err(api::ApiError::Closed),
                Ok(length) => self.buffer.extend_from_slice(&chunk[..length]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(api::ApiError::Io(error)),
            }
        }
        while let Some(newline) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=newline).collect();
            let Ok(line) = std::str::from_utf8(&line) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let Ok(event) = serde_json::from_value::<api::StatusEvent>(
                value.pointer("/data").cloned().unwrap_or_default(),
            ) else {
                continue;
            };
            if event.pane_id != self.pane_id {
                continue;
            }
            let mut snapshot = state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let mut next_mood = None;
            if let Some(subject) = snapshot.subject.as_mut() {
                if let Some(status) = event.agent_status {
                    subject.status = status;
                    next_mood = Some(Mood::from_status(status));
                }
                if event.title.is_some() {
                    subject.title = event.title;
                }
                changed = true;
            }
            if let Some(mood) = next_mood {
                snapshot.mood = mood;
            }
        }
        Ok(changed)
    }
}

fn publish(app: &AppHandle, state: &Arc<Mutex<PetState>>) {
    let snapshot = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let _ = app.emit("pet-state", snapshot);
}

fn run_loop(app: AppHandle, state: Arc<Mutex<PetState>>) {
    let path = api::socket_path();
    let mut reconcile_at = Instant::now();
    let mut subscription: Option<Subscription> = None;

    loop {
        if Instant::now() >= reconcile_at {
            match api::agent_list(&path) {
                Ok(agents) => {
                    let selected = pick_subject(&agents);
                    let wanted_pane = selected.map(|agent| agent.pane_id.clone());
                    {
                        let mut snapshot = state
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        snapshot.subject = selected.map(SubjectInfo::from);
                        snapshot.connection = Connection::Online;
                        snapshot.mood = selected
                            .map(|agent| Mood::from_status(agent.agent_status))
                            .unwrap_or(Mood::Offline);
                        snapshot.offline_reason = None;
                    }
                    publish(&app, &state);
                    let subscribed = subscription.as_ref().is_some_and(|current| {
                        Some(current.pane_id.as_str()) == wanted_pane.as_deref()
                    });
                    if !subscribed {
                        subscription = wanted_pane
                            .as_deref()
                            .and_then(|pane| Subscription::connect(&path, pane).ok());
                    }
                }
                Err(error) => {
                    subscription = None;
                    {
                        let mut snapshot = state
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        snapshot.connection = Connection::Offline;
                        snapshot.mood = Mood::Offline;
                        snapshot.subject = None;
                        snapshot.offline_reason = Some(error.to_string());
                    }
                    publish(&app, &state);
                }
            }
            reconcile_at = Instant::now() + RECONCILE_INTERVAL;
        }

        if let Some(current) = subscription.as_mut() {
            match current.drain(&state) {
                Ok(true) => publish(&app, &state),
                Ok(false) => {}
                Err(_) => subscription = None,
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

fn pick_subject(agents: &[AgentInfo]) -> Option<&AgentInfo> {
    agents.iter().find(|agent| agent.focused).or_else(|| {
        agents.iter().min_by_key(|agent| match agent.agent_status {
            AgentStatus::Blocked => 0,
            AgentStatus::Working => 1,
            AgentStatus::Done => 2,
            AgentStatus::Idle => 3,
            AgentStatus::Unknown => 4,
        })
    })
}

impl From<&AgentInfo> for SubjectInfo {
    fn from(agent: &AgentInfo) -> Self {
        Self {
            pane_id: agent.pane_id.clone(),
            workspace_id: agent.workspace_id.clone(),
            agent: agent.agent.clone().or_else(|| agent.display_agent.clone()),
            title: agent.terminal_title.clone(),
            status: agent.agent_status,
            cwd: agent.cwd.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(pane_id: &str, status: AgentStatus, focused: bool) -> AgentInfo {
        AgentInfo {
            pane_id: pane_id.to_string(),
            workspace_id: "workspace".to_string(),
            agent: None,
            display_agent: None,
            agent_status: status,
            focused,
            terminal_title: None,
            cwd: None,
        }
    }

    #[test]
    fn focused_agent_wins() {
        let agents = [
            agent("blocked", AgentStatus::Blocked, false),
            agent("focused", AgentStatus::Idle, true),
        ];
        assert_eq!(
            pick_subject(&agents).map(|item| item.pane_id.as_str()),
            Some("focused")
        );
    }
}
