use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use interprocess::local_socket::traits::Stream as _;
use tauri::{AppHandle, Emitter};

use crate::api::{self, AgentInfo, AgentStatus};
use crate::state::{Connection, Mood, PetState, SessionInfo, SubjectInfo};
use crate::SharedState;

const RECONCILE_INTERVAL: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(200);

pub enum Control {
    SelectAgent(String),
    SelectSession(String),
}

pub fn start(app: AppHandle, state: Arc<Mutex<SharedState>>) -> Sender<Control> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || run_loop(app, state, receiver));
    sender
}

struct Subscription {
    stream: interprocess::local_socket::Stream,
    pane_ids: HashSet<String>,
    buffer: Vec<u8>,
}

impl Subscription {
    fn connect(path: &std::path::Path, pane_ids: &[String]) -> Result<Self, api::ApiError> {
        let subscriptions = pane_ids
            .iter()
            .map(|pane_id| {
                serde_json::json!({"type": "pane.agent_status_changed", "pane_id": pane_id})
            })
            .collect::<Vec<_>>();
        let request = serde_json::json!({
            "id": "pet:subscribe",
            "method": "events.subscribe",
            "params": {"subscriptions": subscriptions},
        });
        let mut stream = api::connect(path)?;
        stream.set_nonblocking(true).map_err(api::ApiError::Io)?;
        stream.write_all(serde_json::to_string(&request)?.as_bytes())?;
        stream.write_all(b"\n")?;
        stream.flush()?;
        Ok(Self {
            stream,
            pane_ids: pane_ids.iter().cloned().collect(),
            buffer: Vec::new(),
        })
    }

    fn matches(&self, pane_ids: &[String]) -> bool {
        self.pane_ids.len() == pane_ids.len()
            && pane_ids
                .iter()
                .all(|pane_id| self.pane_ids.contains(pane_id))
    }

    fn drain(&mut self) -> Result<Vec<api::StatusEvent>, api::ApiError> {
        let mut chunk = [0_u8; 4096];
        loop {
            match self.stream.read(&mut chunk) {
                Ok(0) => return Err(api::ApiError::Closed),
                Ok(length) => self.buffer.extend_from_slice(&chunk[..length]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(api::ApiError::Io(error)),
            }
        }

        let mut events = Vec::new();
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
            if self.pane_ids.contains(&event.pane_id) {
                events.push(event);
            }
        }
        Ok(events)
    }
}

fn publish(app: &AppHandle, state: &Arc<Mutex<SharedState>>) {
    let snapshot = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .snapshot
        .clone();
    let _ = app.emit("pet-state", snapshot);
}

fn run_loop(app: AppHandle, state: Arc<Mutex<SharedState>>, controls: Receiver<Control>) {
    let mut session = api::active_session();
    let mut path = api::socket_path();
    let mut selected_pane: Option<String> = None;
    let mut status_clock: HashMap<String, (AgentStatus, u64)> = HashMap::new();
    let mut reconcile_at = Instant::now();
    let mut subscription: Option<Subscription> = None;

    loop {
        while let Ok(control) = controls.try_recv() {
            match control {
                Control::SelectAgent(pane_id) => {
                    selected_pane = Some(pane_id);
                    update_subject(&state, selected_pane.as_deref());
                    publish(&app, &state);
                }
                Control::SelectSession(name) => {
                    if let Some(next_path) = api::socket_path_for_session(&name) {
                        session = name;
                        path = next_path;
                        selected_pane = None;
                        status_clock.clear();
                        subscription = None;
                        reconcile_at = Instant::now();
                    }
                }
            }
        }

        if Instant::now() >= reconcile_at {
            reconcile(
                &app,
                &state,
                &path,
                &session,
                &mut selected_pane,
                &mut status_clock,
                &mut subscription,
            );
            reconcile_at = Instant::now() + RECONCILE_INTERVAL;
        }

        if let Some(current) = subscription.as_mut() {
            match current.drain() {
                Ok(events) if !events.is_empty() => {
                    apply_events(&state, events, &mut status_clock, selected_pane.as_deref());
                    publish(&app, &state);
                }
                Ok(_) => {}
                Err(_) => subscription = None,
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

fn reconcile(
    app: &AppHandle,
    state: &Arc<Mutex<SharedState>>,
    path: &std::path::Path,
    session: &str,
    selected_pane: &mut Option<String>,
    status_clock: &mut HashMap<String, (AgentStatus, u64)>,
    subscription: &mut Option<Subscription>,
) {
    let sessions = api::available_sessions(session)
        .into_iter()
        .map(|name| SessionInfo {
            active: name == session,
            name,
        })
        .collect::<Vec<_>>();

    match api::agent_list(path) {
        Ok(agent_data) => {
            let theme = api::client_theme(path).ok().flatten();
            let now = now_ms();
            status_clock
                .retain(|pane_id, _| agent_data.iter().any(|agent| agent.pane_id == *pane_id));
            for agent in &agent_data {
                status_clock
                    .entry(agent.pane_id.clone())
                    .and_modify(|clock| {
                        if clock.0 != agent.agent_status {
                            *clock = (agent.agent_status, now);
                        }
                    })
                    .or_insert((agent.agent_status, now));
            }

            if selected_pane
                .as_ref()
                .is_some_and(|pane| !agent_data.iter().any(|agent| &agent.pane_id == pane))
            {
                *selected_pane = None;
            }
            let selected = selected_pane
                .as_deref()
                .and_then(|pane| agent_data.iter().find(|agent| agent.pane_id == pane))
                .or_else(|| pick_subject(&agent_data));
            let selected_id = selected.map(|agent| agent.pane_id.as_str());
            let agents = agent_data
                .iter()
                .map(|agent| SubjectInfo::from_agent(agent, status_since(status_clock, agent)))
                .collect::<Vec<_>>();
            let subject = selected_id
                .and_then(|pane| agents.iter().find(|agent| agent.pane_id == pane))
                .cloned();
            let pane_ids = agent_data
                .iter()
                .map(|agent| agent.pane_id.clone())
                .collect::<Vec<_>>();

            {
                let mut shared = state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                shared.socket_path = path.to_path_buf();
                shared.snapshot = PetState {
                    connection: Connection::Online,
                    mood: subject
                        .as_ref()
                        .map(|agent| Mood::from_status(agent.status))
                        .unwrap_or(Mood::Sleeping),
                    blocked_count: agents
                        .iter()
                        .filter(|agent| agent.status == AgentStatus::Blocked)
                        .count(),
                    subject,
                    agents,
                    session: session.to_string(),
                    sessions,
                    offline_reason: None,
                    theme,
                };
            }
            publish(app, state);

            if pane_ids.is_empty() {
                *subscription = None;
            } else if !subscription
                .as_ref()
                .is_some_and(|current| current.matches(&pane_ids))
            {
                *subscription = Subscription::connect(path, &pane_ids).ok();
            }
        }
        Err(error) => {
            *subscription = None;
            let mut shared = state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            shared.socket_path = path.to_path_buf();
            shared.snapshot.connection = Connection::Offline;
            if shared.snapshot.session != session {
                shared.snapshot.theme = None;
            }
            shared.snapshot.mood = Mood::Offline;
            shared.snapshot.subject = None;
            shared.snapshot.agents.clear();
            shared.snapshot.blocked_count = 0;
            shared.snapshot.session = session.to_string();
            shared.snapshot.sessions = sessions;
            shared.snapshot.offline_reason = Some(error.to_string());
            drop(shared);
            publish(app, state);
        }
    }
}

fn apply_events(
    state: &Arc<Mutex<SharedState>>,
    events: Vec<api::StatusEvent>,
    status_clock: &mut HashMap<String, (AgentStatus, u64)>,
    selected_pane: Option<&str>,
) {
    let now = now_ms();
    let mut shared = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for event in events {
        let Some(agent) = shared
            .snapshot
            .agents
            .iter_mut()
            .find(|agent| agent.pane_id == event.pane_id)
        else {
            continue;
        };
        if let Some(status) = event.agent_status {
            if agent.status != status {
                agent.status = status;
                agent.status_since_ms = now;
                status_clock.insert(agent.pane_id.clone(), (status, now));
            }
        }
        if event.title.is_some() {
            agent.title = event.title;
        }
    }
    shared.snapshot.blocked_count = shared
        .snapshot
        .agents
        .iter()
        .filter(|agent| agent.status == AgentStatus::Blocked)
        .count();
    let pane_id = selected_pane.map(str::to_owned).or_else(|| {
        shared
            .snapshot
            .subject
            .as_ref()
            .map(|agent| agent.pane_id.clone())
    });
    shared.snapshot.subject = pane_id
        .as_deref()
        .and_then(|pane| {
            shared
                .snapshot
                .agents
                .iter()
                .find(|agent| agent.pane_id == pane)
        })
        .cloned();
    shared.snapshot.mood = shared
        .snapshot
        .subject
        .as_ref()
        .map(|agent| Mood::from_status(agent.status))
        .unwrap_or(Mood::Sleeping);
}

fn update_subject(state: &Arc<Mutex<SharedState>>, selected_pane: Option<&str>) {
    let mut shared = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    shared.snapshot.subject = selected_pane
        .and_then(|pane| {
            shared
                .snapshot
                .agents
                .iter()
                .find(|agent| agent.pane_id == pane)
        })
        .cloned();
    shared.snapshot.mood = shared
        .snapshot
        .subject
        .as_ref()
        .map(|agent| Mood::from_status(agent.status))
        .unwrap_or(Mood::Sleeping);
}

fn status_since(status_clock: &HashMap<String, (AgentStatus, u64)>, agent: &AgentInfo) -> u64 {
    status_clock
        .get(&agent.pane_id)
        .map(|clock| clock.1)
        .unwrap_or_else(now_ms)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
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

impl SubjectInfo {
    fn from_agent(agent: &AgentInfo, status_since_ms: u64) -> Self {
        Self {
            pane_id: agent.pane_id.clone(),
            agent: agent.agent.clone().or_else(|| agent.display_agent.clone()),
            title: agent.terminal_title.clone(),
            status: agent.agent_status,
            cwd: agent.cwd.clone(),
            focused: agent.focused,
            status_since_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(pane_id: &str, status: AgentStatus, focused: bool) -> AgentInfo {
        AgentInfo {
            pane_id: pane_id.to_string(),
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
