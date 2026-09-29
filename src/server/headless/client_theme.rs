use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use crate::api;
use crate::protocol::{endpoint, ServerMessage};

const THEME_READ_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct PendingThemeRead {
    pub(super) api_id: String,
    pub(super) client_id: u64,
    pub(super) respond_to: Sender<String>,
    pub(super) deadline: Instant,
}

fn response(id: String, theme: Option<api::schema::ClientTheme>) -> String {
    serde_json::to_string(&api::schema::SuccessResponse {
        id,
        result: api::schema::ResponseResult::ClientTheme { theme },
    })
    .unwrap_or_default()
}

impl super::HeadlessServer {
    pub(super) fn start_theme_read(&mut self, msg: api::ApiRequestMessage) {
        let Some(client_id) = self.foreground_client_id.filter(|id| {
            self.clients.get(id).is_some_and(|client| {
                client.is_active_shell_client() && client.theme_read && client.writer.is_some()
            })
        }) else {
            let _ = msg.respond_to.send(response(msg.request.id, None));
            return;
        };
        let request_id = format!("theme-{}", self.next_theme_read_id);
        self.next_theme_read_id = self.next_theme_read_id.saturating_add(1);
        let Ok(data) = serde_json::to_string(&endpoint::EndpointClientThemeRequest {
            request_id: request_id.clone(),
        }) else {
            let _ = msg.respond_to.send(response(msg.request.id, None));
            return;
        };
        if !self.send_to_client(
            client_id,
            ServerMessage::EndpointControl {
                kind: endpoint::CLIENT_THEME_GET_KIND.into(),
                data,
            },
        ) {
            let _ = msg.respond_to.send(response(msg.request.id, None));
            return;
        }
        self.pending_theme_reads.insert(
            request_id,
            PendingThemeRead {
                api_id: msg.request.id,
                client_id,
                respond_to: msg.respond_to,
                deadline: Instant::now() + THEME_READ_TIMEOUT,
            },
        );
    }

    pub(super) fn finish_theme_read(&mut self, client_id: u64, data: &str) {
        let Ok(result) = serde_json::from_str::<endpoint::EndpointClientThemeResult>(data) else {
            return;
        };
        if self
            .pending_theme_reads
            .get(&result.request_id)
            .is_none_or(|pending| pending.client_id != client_id)
        {
            return;
        }
        if let Some(pending) = self.pending_theme_reads.remove(&result.request_id) {
            let _ = pending
                .respond_to
                .send(response(pending.api_id, result.theme));
        }
    }

    pub(super) fn expire_theme_reads(&mut self, now: Instant) {
        self.pending_theme_reads.retain(|_, pending| {
            if now < pending.deadline {
                return true;
            }
            let _ = pending
                .respond_to
                .send(response(pending.api_id.clone(), None));
            false
        });
    }

    pub(super) fn cancel_theme_reads_for(&mut self, client_id: u64) {
        self.pending_theme_reads.retain(|_, pending| {
            if pending.client_id != client_id {
                return true;
            }
            let _ = pending
                .respond_to
                .send(response(pending.api_id.clone(), None));
            false
        });
    }
}
