use super::*;

fn read_theme(server: &mut HeadlessServer) -> std::sync::mpsc::Receiver<String> {
    let (respond_to, receiver) = std::sync::mpsc::channel();
    server.start_theme_read(api::ApiRequestMessage {
        request: api::schema::Request {
            id: "read-theme".into(),
            method: api::schema::Method::ClientThemeGet(api::schema::EmptyParams::default()),
        },
        respond_to,
        stream_active: None,
        response_write_complete: None,
    });
    receiver
}

#[test]
fn client_theme_returns_none_without_a_compatible_foreground_client() {
    let mut server = test_headless_server();
    let result: serde_json::Value =
        serde_json::from_str(&read_theme(&mut server).recv().unwrap()).unwrap();
    assert!(result["result"]["theme"].is_null());
    assert!(server.pending_theme_reads.is_empty());

    let (writer, control_rx, _render_rx) = test_client_writer();
    server.clients.insert(
        7,
        ClientConnection::new(
            (80, 24),
            crate::kitty_graphics::HostCellSize::default(),
            1,
            protocol::RenderEncoding::SemanticFrame,
            Some(writer),
        ),
    );
    server.foreground_client_id = Some(7);
    let result: serde_json::Value =
        serde_json::from_str(&read_theme(&mut server).recv().unwrap()).unwrap();
    assert!(result["result"]["theme"].is_null());
    assert!(control_rx.try_recv().is_err());
    assert!(server.pending_theme_reads.is_empty());
}

#[test]
fn client_theme_read_results_are_correlated_and_disconnect_cancels_them() {
    let mut server = test_headless_server();
    let (respond_to, receiver) = std::sync::mpsc::channel();
    server.pending_theme_reads.insert(
        "theme-1".into(),
        super::super::client_theme::PendingThemeRead {
            api_id: "read-theme".into(),
            client_id: 7,
            respond_to,
            deadline: Instant::now() + Duration::from_secs(5),
        },
    );
    server.finish_theme_read(8, r#"{"request_id":"theme-1","theme":null}"#);
    assert!(receiver.try_recv().is_err());
    assert_eq!(server.pending_theme_reads.len(), 1);
    server.cancel_theme_reads_for(7);
    let result: serde_json::Value = serde_json::from_str(&receiver.recv().unwrap()).unwrap();
    assert!(result["result"]["theme"].is_null());
    assert!(server.pending_theme_reads.is_empty());
}

#[test]
fn client_theme_read_round_trip_uses_the_selected_clients_theme() {
    let mut server = test_headless_server();
    let (writer, control_rx, _render_rx) = test_client_writer();
    let mut client = ClientConnection::new(
        (80, 24),
        crate::kitty_graphics::HostCellSize::default(),
        1,
        protocol::RenderEncoding::SemanticFrame,
        Some(writer),
    );
    client.mode = ClientConnectionMode::ClientShell;
    client.theme_read = true;
    server.clients.insert(7, client);
    server.foreground_client_id = Some(7);
    let receiver = read_theme(&mut server);
    let framed = control_rx.recv().unwrap();
    let message: ServerMessage =
        protocol::read_message(&mut std::io::Cursor::new(framed), protocol::MAX_FRAME_SIZE)
            .unwrap();
    let ServerMessage::EndpointControl { kind, data } = message else {
        panic!("theme request");
    };
    assert_eq!(kind, protocol::endpoint::CLIENT_THEME_GET_KIND);
    let request: protocol::endpoint::EndpointClientThemeRequest =
        serde_json::from_str(&data).unwrap();
    let theme = api::schema::ClientTheme {
        name: "custom".into(),
        colors: api::schema::ClientThemeColors {
            accent: Some("#010203".into()),
            ..Default::default()
        },
    };
    server.finish_theme_read(
        7,
        &serde_json::to_string(&protocol::endpoint::EndpointClientThemeResult {
            request_id: request.request_id,
            theme: Some(theme.clone()),
        })
        .unwrap(),
    );
    let result: api::schema::SuccessResponse =
        serde_json::from_str(&receiver.recv().unwrap()).unwrap();
    assert_eq!(result.id, "read-theme");
    assert_eq!(
        result.result,
        api::schema::ResponseResult::ClientTheme { theme: Some(theme) }
    );
    assert!(server.pending_theme_reads.is_empty());
}

#[test]
fn client_theme_read_timeout_returns_unavailable_without_disconnecting() {
    let mut server = test_headless_server();
    let (respond_to, receiver) = std::sync::mpsc::channel();
    server.pending_theme_reads.insert(
        "theme-1".into(),
        super::super::client_theme::PendingThemeRead {
            api_id: "expired".into(),
            client_id: 7,
            respond_to,
            deadline: Instant::now(),
        },
    );
    server.expire_theme_reads(Instant::now());
    let result: serde_json::Value = serde_json::from_str(&receiver.recv().unwrap()).unwrap();
    assert_eq!(result["id"], "expired");
    assert!(result["result"]["theme"].is_null());
    assert!(server.pending_theme_reads.is_empty());
}
