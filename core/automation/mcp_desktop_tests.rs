use super::*;
use std::io::{BufRead, BufReader, Write};

#[test]
fn manual_canvas_resize_preserves_indices_and_pads_without_scaling() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = Desktop::new(dir.path().to_owned(), None).unwrap();
    desktop.create_artwork(2, 2).unwrap();
    {
        let mut state = desktop.server.state.lock().unwrap();
        let mut doc =
            crate::schema::read_document(&state.session.document_snapshot().unwrap()).unwrap();
        doc.palette.push("#FFFFFFFF".into());
        doc.cels = vec![crate::schema::CelDefinition::Legacy(
            crate::schema::LegacyCel {
                id: "cel-1".into(),
                pixels: vec![vec![1, 2], vec![2, 1]],
            },
        )];
        state
            .session
            .restore_document(&serde_json::to_vec(&doc).unwrap())
            .unwrap();
    }
    desktop.begin_turn().unwrap();
    desktop.resize_canvas(3, 4).unwrap();
    assert!(!desktop.can_revert());
    let snapshot = || {
        let state = desktop.server.state.lock().unwrap();
        crate::schema::read_document(&state.session.document_snapshot().unwrap()).unwrap()
    };
    let doc = snapshot();
    assert_eq!(
        crate::schema::decode_indices(&doc).unwrap()["cel-1"],
        vec![vec![1, 2, 0], vec![2, 1, 0], vec![0, 0, 0], vec![0, 0, 0],]
    );
    let before = serde_json::to_vec(&doc).unwrap();
    assert!(desktop.resize_canvas(0, 4).is_err());
    assert!(desktop.resize_canvas(4096, 4096).is_err());
    assert_eq!(serde_json::to_vec(&snapshot()).unwrap(), before);
    desktop.resize_canvas(1, 1).unwrap();
    assert_eq!(
        crate::schema::decode_indices(&snapshot()).unwrap()["cel-1"],
        vec![vec![1]]
    );
    assert!(dir.path().join("artwork.pixen").exists());
}

#[test]
fn failed_revert_retains_its_baseline_for_retry() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pixen");
    let mut session = Session::default();
    session
        .create(CreateProject {
            width: 1,
            height: 1,
            palette: vec!["#00000000".into()],
            output_path: source.clone(),
        })
        .unwrap();
    session.save("project-1").unwrap();
    let workspace = dir.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let desktop = Desktop::new(workspace, Some(&source)).unwrap();
    desktop.begin_turn().unwrap();
    let (_, _, recovery) = desktop.snapshot(None).unwrap();
    std::fs::remove_file(&recovery).unwrap();
    std::fs::create_dir(&recovery).unwrap();
    assert!(desktop.revert().is_err());
    assert!(desktop.can_revert());
    std::fs::remove_dir(&recovery).unwrap();
    desktop.revert().unwrap();
    assert!(!desktop.can_revert());
}

fn request(
    socket: &mut BufReader<std::net::TcpStream>,
    id: u64,
    method: &str,
    params: Value,
) -> Value {
    writeln!(
        socket.get_mut(),
        "{}",
        serde_json::json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
    )
    .unwrap();
    loop {
        let mut line = String::new();
        assert_ne!(
            socket.read_line(&mut line).unwrap(),
            0,
            "bridge disconnected"
        );
        let value: Value = serde_json::from_str(&line).unwrap();
        if value["id"] == id {
            return value;
        }
    }
}

#[test]
fn bridge_edits_the_desktop_document_and_filters_file_actions() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = Desktop::new(dir.path().to_owned(), None).unwrap();
    let bridge = Bridge::start(desktop.clone()).unwrap();
    let mut socket = std::net::TcpStream::connect(bridge.address).unwrap();
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    writeln!(socket, "{}", bridge.token).unwrap();
    let mut socket = BufReader::new(socket);
    let init = request(
        &mut socket,
        1,
        "initialize",
        serde_json::json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}),
    );
    assert!(init.get("error").is_none(), "{init}");
    assert!(init["result"]["capabilities"].get("resources").is_some());
    assert!(init["result"]["capabilities"].get("prompts").is_some());
    writeln!(
        socket.get_mut(),
        "{}",
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    let tools = request(&mut socket, 2, "tools/list", serde_json::json!({}));
    assert!(
        tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "read_guidance")
    );
    // Guidance remains available before an artwork exists and cannot touch files.
    for (id, method, params) in [
        (100, "resources/list", serde_json::json!({})),
        (
            101,
            "resources/read",
            serde_json::json!({"uri":"skill://studio"}),
        ),
        (
            102,
            "resources/read",
            serde_json::json!({"uri":"rules://00-core-principles"}),
        ),
        (103, "prompts/list", serde_json::json!({})),
        (104, "prompts/get", serde_json::json!({"name":"pixen:draw"})),
        (
            105,
            "tools/call",
            serde_json::json!({"name":"read_guidance","arguments":{"uri":"rules://index"}}),
        ),
    ] {
        let response = request(&mut socket, id, method, params);
        assert!(response.get("error").is_none(), "{response}");
        assert_ne!(response["result"]["isError"], true, "{response}");
        assert!(response["result"].to_string().len() > 100, "{response}");
    }
    let failure = request(
        &mut socket,
        106,
        "tools/call",
        serde_json::json!({"name":"read_guidance","arguments":{"uri":"file:///external.txt"}}),
    );
    assert_eq!(failure["result"]["isError"], true);
    assert!(
        desktop
            .server
            .state
            .lock()
            .unwrap()
            .session
            .active_id()
            .is_none()
    );
    assert!(
        tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| allowed(t["name"].as_str().unwrap()))
    );
    let created = request(
        &mut socket,
        3,
        "tools/call",
        serde_json::json!({"name":"sprite_manage","arguments":{"op":"new","width":2,"height":2,"palette":["#00000000","#FFFFFFFF"]}}),
    );
    assert_ne!(created["result"]["isError"], true, "{created}");
    assert!(
        dir.path().join("artwork.pixen").is_file(),
        "temporary project container should exist before drawing"
    );
    let edit = request(
        &mut socket,
        4,
        "tools/call",
        serde_json::json!({"name":"edit_batch","arguments":{"project_id":"project-1","expected_revision":0,"operations":[{"tool":"draw","arguments":{"frame_id":"frame-1","layer_id":"layer-1","operation":{"type":"set_pixels","pixels":[{"x":0,"y":0,"palette_index":1}]}}}]}}),
    );
    assert_ne!(edit["result"]["isError"], true, "{edit}");
    assert_eq!(desktop.begin_turn().unwrap()["revision"], 1);
    let resized = request(
        &mut socket,
        6,
        "tools/call",
        serde_json::json!({
            "name":"resize_canvas", "arguments":{"project_id":"project-1","expected_revision":1,"width":3,"height":4}
        }),
    );
    assert_ne!(resized["result"]["isError"], true, "{resized}");
    let (revision, sprite, _) = desktop.snapshot(None).unwrap();
    assert_eq!((revision, sprite.width, sprite.height), (2, 3, 4));
    assert_eq!(
        &crate::raster::render(&sprite, 0).unwrap()[..4],
        &[255, 255, 255, 255]
    );
    assert!(
        desktop.can_revert(),
        "MCP resizing must preserve the conversational turn baseline"
    );
    let stale = request(
        &mut socket,
        7,
        "tools/call",
        serde_json::json!({
            "name":"resize_canvas", "arguments":{"project_id":"project-1","expected_revision":1,"width":1,"height":1}
        }),
    );
    assert_eq!(stale["result"]["isError"], true);
    assert_eq!(desktop.begin_turn().unwrap()["revision"], 2);
    let reported = request(
        &mut socket,
        8,
        "tools/call",
        serde_json::json!({"name":"report_activity","arguments":{
            "project_id":"project-1","message":"Painting slime highlights","busy":true,"frame_id":"frame-1","layer_id":"layer-1"
        }}),
    );
    assert_ne!(reported["result"]["isError"], true, "{reported}");
    let inspected = request(
        &mut socket,
        9,
        "tools/call",
        serde_json::json!({"name":"inspect_project","arguments":{"project_id":"project-1","frame_id":"frame-1","scale":1}}),
    );
    assert_ne!(inspected["result"]["isError"], true, "{inspected}");
    let events = desktop.drain_events();
    assert!(events.iter().any(|e| matches!(e,DesktopEvent::Activity(a) if a.label=="Painting slime highlights" && a.frame_id.as_deref()==Some("frame-1"))));
    assert!(
        events
            .iter()
            .any(|e| matches!(e,DesktopEvent::Inspection(v) if v.frame_id=="frame-1"))
    );
    let denied = request(
        &mut socket,
        5,
        "tools/call",
        serde_json::json!({"name":"finish_project","arguments":{"project_id":"project-1"}}),
    );
    assert_eq!(denied["result"]["isError"], true);
    assert!(desktop.snapshot(None).is_some());
    drop(bridge);
}

#[test]
fn bridge_rejects_an_incorrect_token() {
    let dir = tempfile::tempdir().unwrap();
    let bridge = Bridge::start(Desktop::new(dir.path().to_owned(), None).unwrap()).unwrap();
    let mut socket = std::net::TcpStream::connect(bridge.address).unwrap();
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    writeln!(socket, "{}", "0".repeat(64)).unwrap();
    let mut byte = [0];
    use std::io::Read;
    assert_eq!(socket.read(&mut byte).unwrap_or(0), 0);
}

#[test]
fn bridge_accepts_pixel_payload_above_256_kib() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = Desktop::new(dir.path().to_owned(), None).unwrap();
    desktop.create_artwork(128, 128).unwrap();
    let bridge = Bridge::start(desktop.clone()).unwrap();
    let mut socket = std::net::TcpStream::connect(bridge.address).unwrap();
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .unwrap();
    writeln!(socket, "{}", bridge.token).unwrap();
    let mut socket = BufReader::new(socket);
    request(
        &mut socket,
        1,
        "initialize",
        serde_json::json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}),
    );
    writeln!(
        socket.get_mut(),
        "{}",
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    let pixels: Vec<_> = (0..16384)
        .map(|i| serde_json::json!({"x":i%128,"y":i/128,"palette_index":1}))
        .collect();
    let params = serde_json::json!({"name":"edit_batch","arguments":{"project_id":"project-1","expected_revision":0,"operations":[{"tool":"draw","arguments":{"frame_id":"frame-1","layer_id":"layer-1","operation":{"type":"set_pixels","pixels":pixels}}}]}});
    assert!(serde_json::to_vec(&params).unwrap().len() > 256 * 1024);
    let result = request(&mut socket, 2, "tools/call", params);
    assert_ne!(result["result"]["isError"], true, "{result}");
    assert!(result.get("error").is_none(), "{result}");
    assert!(
        desktop
            .read_cel("frame-1", "layer-1")
            .unwrap()
            .pixels
            .chunks_exact(4)
            .all(|p| p == [255, 255, 255, 255])
    );
}

#[test]
fn message_checkpoints_restore_artwork_and_discard_later_checkpoints() {
    let root = tempfile::tempdir().unwrap();
    let desktop = Desktop::new(root.path().to_owned(), None).unwrap();
    desktop.create_artwork(2, 2).unwrap();
    desktop.begin_turn().unwrap();
    let first = desktop.checkpoint_turn().unwrap();
    desktop.resize_canvas(3, 3).unwrap();
    desktop.begin_turn().unwrap();
    let second = desktop.checkpoint_turn().unwrap();
    desktop.resize_canvas(4, 4).unwrap();
    assert_eq!(desktop.rewind_count(first).unwrap(), 2);
    desktop.rewind(second).unwrap();
    let (_, sprite, _) = desktop.snapshot(None).unwrap();
    assert_eq!((sprite.width, sprite.height), (3, 3));
    assert!(desktop.rewind_count(second).is_err());
    desktop.rewind(first).unwrap();
    let (_, sprite, _) = desktop.snapshot(None).unwrap();
    assert_eq!((sprite.width, sprite.height), (2, 2));
    assert!(desktop.rewind_count(first).is_err());
}
#[test]
fn desktop_discovery_owns_paths_and_stops_connected_clients_with_the_app() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = Desktop::new(dir.path().to_owned(), None).unwrap();
    let bridge = Bridge::start(desktop.clone()).unwrap();
    let mut socket = std::net::TcpStream::connect(bridge.address).unwrap();
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    writeln!(socket, "{}", bridge.token).unwrap();
    let mut socket = BufReader::new(socket);
    request(
        &mut socket,
        1,
        "initialize",
        serde_json::json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"ownership-test","version":"1"}}),
    );
    writeln!(
        socket.get_mut(),
        "{}",
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    let preflight = request(
        &mut socket,
        2,
        "tools/call",
        serde_json::json!({"name":"preflight","arguments":{}}),
    );
    let result = &preflight["result"]["structuredContent"]["result"];
    assert_eq!(result["mode"], "desktop");
    assert_eq!(result["ready"], true);
    assert_eq!(result["has_project"], false);
    let path = dir.path().join("unauthorized.pixen");
    for (id, name, args) in [
        (
            3,
            "sprite_manage",
            serde_json::json!({"op":"new","width":1,"height":1,"palette":["#FFFFFFFF"],"output_path":path}),
        ),
        (4, "sprite_manage", serde_json::json!({"op":"save"})),
    ] {
        let denied = request(
            &mut socket,
            id,
            "tools/call",
            serde_json::json!({"name":name,"arguments":args}),
        );
        assert_eq!(
            denied["result"]["structuredContent"]["error"]["code"], "application_owned",
            "{denied}"
        );
    }
    assert!(!path.exists());
    assert!(
        desktop
            .server
            .state
            .lock()
            .unwrap()
            .session
            .active_id()
            .is_none()
    );
    let created = request(
        &mut socket,
        5,
        "tools/call",
        serde_json::json!({"name":"sprite_manage","arguments":{"op":"new","width":1,"height":1,"palette":["#FFFFFFFF"]}}),
    );
    assert_eq!(
        created["result"]["structuredContent"]["revision"], 0,
        "{created}"
    );
    let renamed = request(
        &mut socket,
        6,
        "tools/call",
        serde_json::json!({"name":"frame","arguments":{"op":"rename","expected_revision":0,"name":"pose"}}),
    );
    assert_eq!(
        renamed["result"]["structuredContent"]["revision"], 1,
        "{renamed}"
    );
    let listed = request(
        &mut socket,
        7,
        "tools/call",
        serde_json::json!({"name":"frame","arguments":{"op":"list"}}),
    );
    assert_eq!(
        listed["result"]["structuredContent"]["result"]["frames"][0]["name"],
        "pose"
    );
    drop(bridge);
    let mut line = String::new();
    assert_eq!(
        socket.read_line(&mut line).unwrap_or(0),
        0,
        "client remained connected after application shutdown"
    );
}
#[test]
fn temporary_container_failure_reports_a_committed_edit_as_success() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = Desktop::new(dir.path().to_owned(), None).unwrap();
    desktop.create_artwork(1, 1).unwrap();
    std::fs::remove_file(dir.path().join("artwork.pixen")).unwrap();
    std::fs::create_dir(dir.path().join("artwork.pixen")).unwrap();
    let bridge = Bridge::start(desktop.clone()).unwrap();
    let mut socket = std::net::TcpStream::connect(bridge.address).unwrap();
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    writeln!(socket, "{}", bridge.token).unwrap();
    let mut socket = BufReader::new(socket);
    request(
        &mut socket,
        1,
        "initialize",
        serde_json::json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"persistence-test","version":"1"}}),
    );
    writeln!(
        socket.get_mut(),
        "{}",
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    let edited = request(
        &mut socket,
        2,
        "tools/call",
        serde_json::json!({"name":"layer","arguments":{"op":"create","name":"ink","expected_revision":0}}),
    );
    assert_ne!(edited["result"]["isError"], true, "{edited}");
    assert_eq!(edited["result"]["structuredContent"]["revision"], 1);
    assert!(edited["result"]["structuredContent"]["result"]["persistence_warning"].is_string());
    assert_eq!(desktop.begin_turn().unwrap()["revision"], 1);
}
