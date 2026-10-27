use pixen::desktop::{Application, RenderRequest};

#[test]
fn recovery_timer_runs_without_frontend_polling_and_does_not_save_the_project() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("project.pixloom");
    let mut app = Application::new(false);
    app.create(1, 1).unwrap();
    app.save(Some(&path)).unwrap();
    let original = std::fs::read(&path).unwrap();
    app.configure_recovery(1).unwrap();
    let doc = app.snapshot().unwrap().document.unwrap();
    app.paint(
        doc.generation,
        &doc.frames[0].id,
        &doc.layers[0].id,
        vec![pixen::desktop::PaintPixel {
            x: 0,
            y: 0,
            color: "#ff0000ff".into(),
        }],
    )
    .unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(6);
    // Reading status does not trigger either document refresh or recovery scheduling.
    while app.recovery_status().unwrap().revision != Some(1) {
        assert!(std::time::Instant::now() < until);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(app.snapshot().unwrap().document.unwrap().dirty);
    assert!(app.configure_recovery(0).is_err());
}

#[test]
fn desktop_edits_use_memory_until_a_background_recovery_checkpoint() {
    let mut app = Application::new(false);
    app.create(2, 2).unwrap();
    app.configure_recovery(3600).unwrap();
    let desktop = app.ai_desktop().unwrap();
    let (initial, _, path) = desktop.snapshot(None).unwrap();
    let before = std::fs::read(&path).unwrap();
    let doc = app.snapshot().unwrap().document.unwrap();
    let frame = &doc.frames[0].id;
    let layer = &doc.layers[0].id;
    app.paint(
        doc.generation,
        frame,
        layer,
        vec![pixen::desktop::PaintPixel {
            x: 0,
            y: 0,
            color: "#ff0000ff".into(),
        }],
    )
    .unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "editing must not write recovery"
    );
    assert!(
        desktop.snapshot(Some(initial)).is_some(),
        "desktop observes in-memory edits before recovery is written"
    );
    assert_eq!(&app.cel(frame, layer).unwrap()[..4], &[255, 0, 0, 255]);
    assert!(app.recover_now().unwrap());
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.recovery_status().unwrap().writing {
        assert!(std::time::Instant::now() < until);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(app.recovery_status().unwrap().revision, Some(1));
    assert!(app.recovery_status().unwrap().saved_at.is_some());
    assert_ne!(std::fs::read(&path).unwrap(), before);
    let mut restored = Application::new(false);
    restored.open(&path).unwrap();
    assert_eq!(&restored.cel(frame, layer).unwrap()[..4], &[255, 0, 0, 255]);
    assert!(
        restored.history(false).is_err(),
        "recovery must not persist undo history"
    );
}

#[test]
fn captured_save_does_not_save_newer_edits_or_clear_their_dirty_state() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("async.pixloom");
    let mut app = Application::new(false);
    app.create(2, 2).unwrap();
    let doc = app.snapshot().unwrap().document.unwrap();
    let frame = &doc.frames[0].id;
    let layer = &doc.layers[0].id;
    app.paint(
        doc.generation,
        frame,
        layer,
        vec![pixen::desktop::PaintPixel {
            x: 0,
            y: 0,
            color: "#ff0000ff".into(),
        }],
    )
    .unwrap();
    let captured = app.prepare_save(Some(&path)).unwrap();
    assert_eq!(captured.state.revision, 1);
    let generation = app.snapshot().unwrap().document.unwrap().generation;
    app.paint(
        generation,
        frame,
        layer,
        vec![pixen::desktop::PaintPixel {
            x: 0,
            y: 0,
            color: "#00ff00ff".into(),
        }],
    )
    .unwrap();
    let saved = std::thread::spawn(move || captured.write())
        .join()
        .unwrap()
        .unwrap();
    app.finish_save(&saved).unwrap();
    assert!(app.snapshot().unwrap().document.unwrap().dirty);
    assert_eq!(&app.cel(frame, layer).unwrap()[..4], &[0, 255, 0, 255]);
    let mut reopened = Application::new(false);
    reopened.open(&path).unwrap();
    assert_eq!(&reopened.cel(frame, layer).unwrap()[..4], &[255, 0, 0, 255]);
    app.save(None).unwrap();
    assert!(!app.snapshot().unwrap().document.unwrap().dirty);
    reopened.open(&path).unwrap();
    assert_eq!(&reopened.cel(frame, layer).unwrap()[..4], &[0, 255, 0, 255]);
}

#[test]
fn matrix_snapshot_and_native_paint_preserve_linked_cells() {
    use serde_json::json;
    let mut app = Application::new(false);
    app.create(2, 2).unwrap();
    let doc = app.snapshot().unwrap().document.unwrap();
    let frame = doc.frames[0].id.clone();
    let layer = doc.layers[0].id.clone();
    app.edit_structure(doc.generation, serde_json::from_value(json!([
        {"type":"add_frame","frame_id":"next","duration_ms":100},
        {"type":"link_cells","layer_id":layer,"source_frame_id":frame,"frame_ids":["next"],"link_id":"shared"}
    ])).unwrap()).unwrap();
    let doc = app.snapshot().unwrap().document.unwrap();
    assert_eq!(doc.layers[0].cells.len(), 2);
    assert!(
        doc.layers[0]
            .cells
            .iter()
            .all(|e| e.link.as_deref() == Some("shared"))
    );
    assert_eq!(doc.layers[0].cells[0].cel, doc.layers[0].cells[1].cel);
    app.paint(
        doc.generation,
        "next",
        &layer,
        vec![pixen::desktop::PaintPixel {
            x: 0,
            y: 0,
            color: "#ff0000ff".into(),
        }],
    )
    .unwrap();
    assert_eq!(&app.cel(&frame, &layer).unwrap()[..4], &[255, 0, 0, 255]);
    let doc = app.snapshot().unwrap().document.unwrap();
    assert!(doc.layers[0].cells[0].cel.is_some());
    assert_eq!(doc.layers[0].cells[0].cel, doc.layers[0].cells[1].cel);
    app.edit_structure(
        doc.generation,
        serde_json::from_value(
            json!({"type":"unlink_cells","layer_id":layer,"frame_ids":["next"]}),
        )
        .unwrap(),
    )
    .unwrap();
    let doc = app.snapshot().unwrap().document.unwrap();
    assert!(doc.layers[0].cells[1].link.is_none());
    app.paint(
        doc.generation,
        "next",
        &layer,
        vec![pixen::desktop::PaintPixel {
            x: 0,
            y: 0,
            color: "#0000ffff".into(),
        }],
    )
    .unwrap();
    assert_eq!(&app.cel(&frame, &layer).unwrap()[..4], &[255, 0, 0, 255]);
    assert_eq!(&app.cel("next", &layer).unwrap()[..4], &[0, 0, 255, 255]);
}

#[test]
fn mcp_review_after_an_edit_is_fresh_until_the_next_edit() {
    use serde_json::{Value, json};
    use std::io::{BufRead, BufReader, Write};
    fn request(
        socket: &mut BufReader<std::net::TcpStream>,
        id: u64,
        method: &str,
        params: Value,
    ) -> Value {
        writeln!(
            socket.get_mut(),
            "{}",
            json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        )
        .unwrap();
        loop {
            let mut line = String::new();
            assert!(socket.read_line(&mut line).unwrap() > 0);
            let value: Value = serde_json::from_str(&line).unwrap();
            if value["id"] == id {
                return value;
            }
        }
    }
    let mut app = Application::new(false);
    app.create(2, 2).unwrap();
    let desktop = app.ai_desktop().unwrap();
    let project = desktop.begin_turn().unwrap()["project_id"].clone();
    let bridge = pixen::mcp::desktop::Bridge::start(desktop.clone()).unwrap();
    let mut tcp = std::net::TcpStream::connect(bridge.address).unwrap();
    tcp.set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    writeln!(tcp, "{}", bridge.token).unwrap();
    let mut socket = BufReader::new(tcp);
    request(
        &mut socket,
        1,
        "initialize",
        json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}),
    );
    writeln!(
        socket.get_mut(),
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    desktop.resize_canvas(3, 3).unwrap();
    let review = request(
        &mut socket,
        2,
        "tools/call",
        json!({"name":"get_review_sheet","arguments":{"project_id":project,"frame_id":"frame-1"}}),
    );
    assert_ne!(review["result"]["isError"], true, "{review}");
    let snapshot = app.snapshot().unwrap();
    assert!(snapshot.review.is_some());
    assert!(!snapshot.review_stale);
    desktop.resize_canvas(4, 4).unwrap();
    assert!(app.snapshot().unwrap().review_stale);
}

#[test]
fn ai_edits_share_the_manual_canvas_and_undo_history() {
    let mut app = Application::new(false);
    app.create(2, 2).unwrap();
    let before = app.snapshot().unwrap().document.unwrap();
    let desktop = app.ai_desktop().unwrap();
    desktop
        .paint(
            before.revision,
            &before.frames[0].id,
            &before.layers[0].id,
            vec![pixen::desktop::PaintPixel {
                x: 0,
                y: 0,
                color: "#ff0000ff".into(),
            }],
        )
        .unwrap();
    let after = app.snapshot().unwrap().document.unwrap();
    assert!(after.generation > before.generation);
    assert!(after.dirty);
    assert_eq!(
        &app.cel(&before.frames[0].id, &before.layers[0].id).unwrap()[..4],
        &[255, 0, 0, 255]
    );
    app.history(false).unwrap();
    assert_eq!(
        &app.cel(&before.frames[0].id, &before.layers[0].id).unwrap()[..4],
        &[0, 0, 0, 0]
    );
    assert!(Application::new(true).ai_desktop().is_err());
}

#[test]
fn duplicates_are_independent_and_group_reorder_is_one_transaction() {
    use serde_json::json;
    fn edit(app: &mut Application, operations: serde_json::Value) {
        let generation = app.snapshot().unwrap().document.unwrap().generation;
        app.edit_structure(generation, serde_json::from_value(operations).unwrap())
            .unwrap();
    }
    let mut app = Application::new(false);
    app.create(2, 2).unwrap();
    let original = app.snapshot().unwrap().document.unwrap();
    let layer = &original.layers[0].id;
    let frame = &original.frames[0].id;
    app.paint(
        original.generation,
        frame,
        layer,
        vec![pixen::desktop::PaintPixel {
            x: 0,
            y: 0,
            color: "#ff0000ff".into(),
        }],
    )
    .unwrap();
    edit(
        &mut app,
        json!({"type":"duplicate_layer","layer_id":layer,"new_layer_id":"copy","name":"Layer copy","index":0}),
    );
    assert_eq!(&app.cel(frame, "copy").unwrap()[..4], &[255, 0, 0, 255]);
    let doc = app.snapshot().unwrap().document.unwrap();
    app.paint(
        doc.generation,
        frame,
        "copy",
        vec![pixen::desktop::PaintPixel {
            x: 0,
            y: 0,
            color: "#00ff00ff".into(),
        }],
    )
    .unwrap();
    assert_eq!(&app.cel(frame, layer).unwrap()[..4], &[255, 0, 0, 255]);
    edit(
        &mut app,
        json!([
            {"type":"duplicate_frame","frame_id":frame,"new_frame_id":"next","index":1},
            {"type":"rename_frame","frame_id":"next","name":"Next copy"}
        ]),
    );
    app.history(false).unwrap();
    assert_eq!(app.snapshot().unwrap().document.unwrap().frames.len(), 1);
    app.history(true).unwrap();
    assert_eq!(
        app.snapshot().unwrap().document.unwrap().frames[1]
            .name
            .as_deref(),
        Some("Next copy")
    );
    edit(
        &mut app,
        json!({"type":"add_layer","layer_id":"third","name":"Third"}),
    );
    edit(
        &mut app,
        json!([
            {"type":"reorder_layer","layer_id":"third","index":0},
            {"type":"reorder_layer","layer_id":layer,"index":1},
            {"type":"reorder_layer","layer_id":"copy","index":2}
        ]),
    );
    let doc = app.snapshot().unwrap().document.unwrap();
    assert_eq!(
        doc.layers.iter().map(|l| l.id.as_str()).collect::<Vec<_>>(),
        vec!["third", layer.as_str(), "copy"]
    );
    app.history(false).unwrap();
    let doc = app.snapshot().unwrap().document.unwrap();
    assert_eq!(
        doc.layers.iter().map(|l| l.id.as_str()).collect::<Vec<_>>(),
        vec!["copy", layer.as_str(), "third"]
    );
    // A bad member rolls back the whole reorder batch.
    assert!(
        app.edit_structure(
            doc.generation,
            serde_json::from_value(json!([
                {"type":"reorder_layer","layer_id":"third","index":0},
                {"type":"remove_layer","layer_id":"missing"}
            ]))
            .unwrap()
        )
        .is_err()
    );
    assert_eq!(
        app.snapshot().unwrap().document.unwrap().layers[0].id,
        "copy"
    );
}

#[test]
fn desktop_structure_edits_preserve_artwork_names_and_history() {
    fn edit(app: &mut Application, operation: serde_json::Value) {
        let generation = app.snapshot().unwrap().document.unwrap().generation;
        app.edit_structure(generation, serde_json::from_value(operation).unwrap())
            .unwrap();
    }
    use serde_json::json;
    let mut app = Application::new(false);
    app.create(2, 2).unwrap();
    let first = app.snapshot().unwrap().document.unwrap();
    let frame = first.frames[0].id.clone();
    let layer = first.layers[0].id.clone();
    app.paint(
        first.generation,
        &frame,
        &layer,
        vec![pixen::desktop::PaintPixel {
            x: 0,
            y: 0,
            color: "#ff0000ff".into(),
        }],
    )
    .unwrap();
    edit(
        &mut app,
        json!({"type":"add_layer","layer_id":"ink","name":"Ink"}),
    );
    edit(
        &mut app,
        json!({"type":"rename_layer","layer_id":"ink","name":"Outline"}),
    );
    edit(
        &mut app,
        json!({"type":"reorder_layer","layer_id":"ink","index":0}),
    );
    edit(
        &mut app,
        json!({"type":"add_frame","frame_id":"next","duration_ms":150}),
    );
    edit(
        &mut app,
        json!({"type":"rename_frame","frame_id":"next","name":"Landing"}),
    );
    edit(
        &mut app,
        json!({"type":"reorder_frame","frame_id":"next","index":0}),
    );
    let doc = app.snapshot().unwrap().document.unwrap();
    assert_eq!(doc.frames[0].name.as_deref(), Some("Landing"));
    assert_eq!(doc.layers[0].name, "Outline");
    assert_eq!(&app.cel(&frame, &layer).unwrap()[..4], &[255, 0, 0, 255]);
    assert!(app.cel("next", "ink").unwrap().iter().all(|v| *v == 0));
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("structure.pixloom");
    app.save(Some(&path)).unwrap();
    let mut reopened = Application::new(false);
    reopened.open(&path).unwrap();
    assert_eq!(
        reopened.snapshot().unwrap().document.unwrap().frames[0]
            .name
            .as_deref(),
        Some("Landing")
    );
    edit(&mut app, json!({"type":"remove_frame","frame_id":"next"}));
    edit(&mut app, json!({"type":"remove_layer","layer_id":"ink"}));
    app.history(false).unwrap();
    app.history(false).unwrap();
    assert_eq!(
        app.snapshot().unwrap().document.unwrap().frames[0]
            .name
            .as_deref(),
        Some("Landing")
    );
    app.history(true).unwrap();
    app.history(true).unwrap();
    let doc = app.snapshot().unwrap().document.unwrap();
    assert_eq!(doc.frames.len(), 1);
    assert_eq!(doc.layers.len(), 1);
    for op in [
        json!({"type":"remove_frame","frame_id":frame}),
        json!({"type":"remove_layer","layer_id":layer}),
        json!({"type":"rename_frame","frame_id":frame,"name":" "}),
    ] {
        assert!(
            app.edit_structure(doc.generation, serde_json::from_value(op).unwrap())
                .is_err()
        );
    }
    assert!(
        app.edit_structure(
            first.generation,
            serde_json::from_value(json!({"type":"rename_layer","layer_id":layer,"name":"Stale"}))
                .unwrap()
        )
        .is_err()
    );
    let mut managed = Application::new(true);
    managed.open(&path).unwrap();
    let generation = managed.snapshot().unwrap().document.unwrap().generation;
    assert!(
        managed
            .edit_structure(
                generation,
                serde_json::from_value(json!({"type":"remove_layer","layer_id":"ink"})).unwrap()
            )
            .is_err()
    );
}

#[test]
fn manual_strokes_are_atomic_revision_checked_and_undoable() {
    let mut app = Application::new(false);
    app.create(2, 2).unwrap();
    let doc = app.snapshot().unwrap().document.unwrap();
    let frame = &doc.frames[0].id;
    let layer = &doc.layers[0].id;
    let pixels = vec![pixen::desktop::PaintPixel {
        x: 1,
        y: 0,
        color: "#ff000080".into(),
    }];
    app.paint(doc.generation, frame, layer, pixels.clone())
        .unwrap();
    assert!(app.paint(doc.generation, frame, layer, pixels).is_err());
    assert_eq!(&app.cel(frame, layer).unwrap()[4..8], &[255, 0, 0, 128]);
    app.history(false).unwrap();
    assert_eq!(&app.cel(frame, layer).unwrap()[4..8], &[0, 0, 0, 0]);
    app.history(true).unwrap();
    assert_eq!(&app.cel(frame, layer).unwrap()[4..8], &[255, 0, 0, 128]);
    let before = app.snapshot().unwrap().document.unwrap().generation;
    assert!(
        app.paint(
            before,
            frame,
            layer,
            vec![pixen::desktop::PaintPixel {
                x: 9,
                y: 0,
                color: "#ffffffff".into()
            }]
        )
        .is_err()
    );
    assert_eq!(app.snapshot().unwrap().document.unwrap().generation, before);
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("manual-stroke.pixloom");
    app.save(Some(&path)).unwrap();
    let mut reopened = Application::new(false);
    reopened.open(&path).unwrap();
    assert_eq!(
        &reopened.cel(frame, layer).unwrap()[4..8],
        &[255, 0, 0, 128]
    );
}

#[test]
fn document_lifetime_save_and_preview_do_not_require_a_frontend() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("saved.pixloom");
    let mut app = Application::new(false);
    app.create(4, 3).unwrap();
    let state = app.snapshot().unwrap();
    let document = state.document.unwrap();
    assert_eq!((document.width, document.height), (4, 3));
    let revision = document.revision;
    let request = RenderRequest {
        generation: document.generation,
        frame_id: document.frames[0].id.clone(),
        hidden_layers: document.layers.iter().map(|l| l.id.clone()).collect(),
        ..Default::default()
    };
    let image = app.render(request).unwrap();
    assert_eq!((image.width, image.height), (4, 3));
    assert_eq!(app.snapshot().unwrap().document.unwrap().revision, revision);
    app.save(Some(&path)).unwrap();
    app.resize(6, 5).unwrap();
    app.save(None).unwrap();
    let mut reopened = Application::new(false);
    reopened.open(&path).unwrap();
    let doc = reopened.snapshot().unwrap().document.unwrap();
    assert_eq!((doc.width, doc.height), (6, 5));
}

#[test]
fn managed_view_is_read_only_and_rejects_stale_render_requests() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source.pixloom");
    let mut writer = Application::new(false);
    writer.create(2, 2).unwrap();
    writer.save(Some(&path)).unwrap();
    let mut app = Application::new(true);
    app.open(&path).unwrap();
    assert!(app.create(4, 4).is_err());
    assert!(app.resize(4, 4).is_err());
    assert!(app.save(None).is_err());
    let doc = app.snapshot().unwrap().document.unwrap();
    assert!(
        app.render(RenderRequest {
            generation: doc.generation + 1,
            frame_id: doc.frames[0].id.clone(),
            ..Default::default()
        })
        .is_err()
    );
    app.message(r#"{"label":"Drawing","frame_id":null,"layer_id":null,"busy":true}"#)
        .unwrap();
    assert!(app.snapshot().unwrap().activity.unwrap().busy);
    writer.resize(3, 4).unwrap();
    writer.save(None).unwrap();
    let updated = app.snapshot().unwrap().document.unwrap();
    assert_eq!((updated.width, updated.height), (3, 4));
    assert!(updated.generation > doc.generation);
}

#[test]
fn view_conversion_never_changes_the_saved_document() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("strip.png");
    let mut image = image::RgbaImage::new(32, 16);
    for y in 3..13 {
        for x in 3..13 {
            image.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            image.put_pixel(x + 16, y, image::Rgba([0, 255, 0, 255]));
        }
    }
    image.save(&path).unwrap();
    let mut app = Application::new(false);
    app.open(&path).unwrap();
    let before = app.snapshot().unwrap().document.unwrap();
    assert_eq!(before.frames.len(), 1);
    app.toggle_view().unwrap();
    let after = app.snapshot().unwrap().document.unwrap();
    assert_eq!((after.width, after.height, after.frames.len()), (16, 16, 2));
    assert_eq!(before.revision, after.revision);
    assert_eq!(app.follow_target("frame-2").unwrap(), "found");
    assert_eq!(app.follow_target(&before.frames[0].id).unwrap(), "changed");
    assert_eq!(app.snapshot().unwrap().document.unwrap().width, 32);
}

#[test]
fn inspection_review_and_resize_preview_preserve_source_pixels() {
    use base64::Engine;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("art.png");
    image::RgbaImage::from_pixel(4, 3, image::Rgba([255, 0, 0, 255]))
        .save(&path)
        .unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut app = Application::new(false);
    app.open(&path).unwrap();
    let doc = app.snapshot().unwrap().document.unwrap();
    let frame = doc.frames[0].id.clone();
    let review = app.local_review("analyze", &frame, None).unwrap();
    assert_eq!(review.report["kind"], "sprite_analysis");
    assert!(!app.snapshot().unwrap().review_stale);
    let rendered = app
        .render(RenderRequest {
            generation: doc.generation,
            frame_id: frame.clone(),
            canvas_width: Some(6),
            canvas_height: Some(5),
            ..Default::default()
        })
        .unwrap();
    let image = image::load_from_memory(
        &base64::engine::general_purpose::STANDARD
            .decode(rendered.png)
            .unwrap(),
    )
    .unwrap()
    .into_rgba8();
    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
    assert_eq!(image.get_pixel(5, 4).0, [0, 0, 0, 0]);
    let hidden = app
        .render(RenderRequest {
            generation: doc.generation,
            frame_id: frame,
            hidden_layers: doc.layers.iter().map(|l| l.id.clone()).collect(),
            ..Default::default()
        })
        .unwrap();
    let image = image::load_from_memory(
        &base64::engine::general_purpose::STANDARD
            .decode(hidden.png)
            .unwrap(),
    )
    .unwrap()
    .into_rgba8();
    assert!(image.pixels().all(|p| p.0[3] == 0));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    app.resize(5, 4).unwrap();
    assert!(app.snapshot().unwrap().review_stale);
}

#[test]
fn external_edits_do_not_overwrite_unsaved_document_changes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("art.pixloom");
    let mut app = Application::new(false);
    app.create(2, 2).unwrap();
    app.save(Some(&path)).unwrap();
    app.resize(3, 3).unwrap();
    let mut other = Application::new(false);
    other.open(&path).unwrap();
    other.resize(4, 4).unwrap();
    other.save(None).unwrap();
    let external = std::fs::read(&path).unwrap();
    assert!(app.snapshot().is_err());
    assert!(app.save(None).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), external);
    let copy = root.path().join("copy.pixloom");
    app.save(Some(&copy)).unwrap();
    let sprite = pixen::preview::load(&copy).unwrap();
    assert_eq!((sprite.width, sprite.height), (3, 3));
}

#[test]
fn json_export_captures_unsaved_pixels_without_changing_project_state() {
    let root = tempfile::tempdir().unwrap();
    let mut app = Application::new(false);
    app.create(2, 2).unwrap();
    let path=root.path().join("project.pixloom");
    app.save(Some(&path)).unwrap();
    let doc=app.snapshot().unwrap().document.unwrap();
    app.paint(doc.generation,&doc.frames[0].id,&doc.layers[0].id,vec![pixen::desktop::PaintPixel{x:0,y:0,color:"#ff0000ff".into()}]).unwrap();
    let before=app.snapshot().unwrap().document.unwrap();
    let document=app.export_document().unwrap();
    let output=root.path().join("export.pixen.json");
    let prepared=pixen::output::Prepared::new(&output,|f|pixen::output::write_document(f,&document)).unwrap();
    prepared.commit(false).unwrap();
    let loaded=pixen::input::compile(&output).unwrap();
    assert_eq!(&pixen::raster::render(&loaded,0).unwrap()[..4],&[255,0,0,255]);
    let after=app.snapshot().unwrap().document.unwrap();
    assert_eq!(before.path,after.path);
    assert_eq!(before.revision,after.revision);
    assert!(after.dirty);
    let mut reopened=Application::new(false);
    reopened.open(&output).unwrap();
    assert_eq!(reopened.snapshot().unwrap().document.unwrap().path,Some(output.canonicalize().unwrap()));
}
