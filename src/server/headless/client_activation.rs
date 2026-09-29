use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use crate::api;
use crate::protocol::{self, ServerMessage};

const CLIENT_ACTIVATION_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) struct PendingClientActivation {
    api_request_id: String,
    control_request_id: String,
    client_id: u64,
    respond_to: Sender<String>,
    deadline: Instant,
}

fn response(id: String, reason: api::schema::ClientActivationReason) -> String {
    serde_json::to_string(&api::schema::SuccessResponse {
        id,
        result: api::schema::ResponseResult::ClientActivation {
            activated: reason == api::schema::ClientActivationReason::Activated,
            reason,
        },
    })
    .unwrap_or_else(|_| "{}".to_owned())
}

impl super::HeadlessServer {
    pub(super) fn start_client_activation(&mut self, msg: api::ApiRequestMessage) {
        use api::schema::ClientActivationReason;

        if self.pending_client_activation.is_some() {
            let _ = msg
                .respond_to
                .send(response(msg.request.id, ClientActivationReason::Busy));
            return;
        }
        let Some(client_id) = self.foreground_client_id.filter(|client_id| {
            self.clients
                .get(client_id)
                .is_some_and(|client| client.is_active_shell_client() && client.writer.is_some())
        }) else {
            let _ = msg.respond_to.send(response(
                msg.request.id,
                ClientActivationReason::NoForegroundClient,
            ));
            return;
        };

        let control_request_id = format!("activate-{}", self.next_client_activation_id);
        self.next_client_activation_id = self.next_client_activation_id.saturating_add(1);
        let data =
            match serde_json::to_string(&protocol::endpoint::EndpointHostWindowActivationRequest {
                request_id: control_request_id.clone(),
            }) {
                Ok(data) => data,
                Err(_) => {
                    let _ = msg
                        .respond_to
                        .send(response(msg.request.id, ClientActivationReason::Failed));
                    return;
                }
            };
        if !self.send_to_client(
            client_id,
            ServerMessage::EndpointControl {
                kind: protocol::endpoint::HOST_WINDOW_ACTIVATE_KIND.into(),
                data,
            },
        ) {
            let _ = msg.respond_to.send(response(
                msg.request.id,
                ClientActivationReason::ClientUnavailable,
            ));
            return;
        }
        self.pending_client_activation = Some(PendingClientActivation {
            api_request_id: msg.request.id,
            control_request_id,
            client_id,
            respond_to: msg.respond_to,
            deadline: Instant::now() + CLIENT_ACTIVATION_TIMEOUT,
        });
    }

    pub(super) fn finish_client_activation(&mut self, client_id: u64, data: &str) {
        let Ok(result) =
            serde_json::from_str::<protocol::endpoint::EndpointHostWindowActivationResult>(data)
        else {
            return;
        };
        let Some(pending) = self.pending_client_activation.as_ref() else {
            return;
        };
        if pending.client_id != client_id || pending.control_request_id != result.request_id {
            return;
        }
        let Some(pending) = self.pending_client_activation.take() else {
            return;
        };
        let reason = match (result.activated, result.reason) {
            (true, api::schema::ClientActivationReason::Activated) => {
                api::schema::ClientActivationReason::Activated
            }
            (false, api::schema::ClientActivationReason::Activated) => {
                api::schema::ClientActivationReason::Failed
            }
            (_, reason) => reason,
        };
        let _ = pending
            .respond_to
            .send(response(pending.api_request_id, reason));
    }

    pub(super) fn client_activation_deadline(&self) -> Option<Instant> {
        self.pending_client_activation
            .as_ref()
            .map(|pending| pending.deadline)
    }

    pub(super) fn expire_client_activation(&mut self, now: Instant) {
        if self
            .pending_client_activation
            .as_ref()
            .is_none_or(|pending| now < pending.deadline)
        {
            return;
        }
        let Some(pending) = self.pending_client_activation.take() else {
            return;
        };
        let _ = pending.respond_to.send(response(
            pending.api_request_id,
            api::schema::ClientActivationReason::TimedOut,
        ));
    }

    pub(super) fn cancel_client_activation_for(&mut self, client_id: u64) {
        if self
            .pending_client_activation
            .as_ref()
            .is_none_or(|pending| pending.client_id != client_id)
        {
            return;
        }
        let Some(pending) = self.pending_client_activation.take() else {
            return;
        };
        let _ = pending.respond_to.send(response(
            pending.api_request_id,
            api::schema::ClientActivationReason::ClientUnavailable,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_response_reports_only_confirmed_activation() {
        let value: serde_json::Value = serde_json::from_str(&response(
            "request".into(),
            api::schema::ClientActivationReason::PermissionDenied,
        ))
        .unwrap();
        assert_eq!(value["result"]["activated"], false);
        assert_eq!(value["result"]["reason"], "permission_denied");
    }
}
