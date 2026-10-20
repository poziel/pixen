use pixloom::session::{CreateProject, Session};
use serde_json::json;

#[test]
fn resizing_canvas_preserves_indices_and_is_atomic_and_undoable() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = Session::default();
    session
        .create(CreateProject {
            width: 2,
            height: 2,
            palette: vec!["#FFFFFFFF".into(), "#FFFFFFFF".into()],
            output_path: dir.path().join("resize.pixloom"),
        })
        .unwrap();
    session
        .apply(
            "project-1",
            0,
            serde_json::from_value(
                json!([{"type":"set_pixels","frame_id":"frame-1","layer_id":"layer-1",
        "pixels":[{"x":0,"y":0,"palette_index":0},{"x":1,"y":1,"palette_index":1}]}]),
            )
            .unwrap(),
        )
        .unwrap();
    let before = session.document_snapshot().unwrap();
    assert_eq!(
        session.resize_canvas("project-1", 1, 3, 4).unwrap()["revision"],
        2
    );
    let doc = pixloom::schema::read_document(&session.document_snapshot().unwrap()).unwrap();
    let grids = pixloom::schema::decode_indices(&doc).unwrap();
    assert_eq!(grids["cel-1"][0][0], 0);
    assert_eq!(grids["cel-1"][1][1], 1);
    assert_eq!(grids["cel-1"][3][2], 2);
    let resized = session.document_snapshot().unwrap();
    assert!(session.resize_canvas("project-1", 1, 1, 1).is_err());
    assert!(session.resize_canvas("project-1", 2, 0, 4).is_err());
    assert_eq!(session.document_snapshot().unwrap(), resized);
    session.undo("project-1", 2).unwrap();
    assert_eq!(session.document_snapshot().unwrap(), before);
    session.redo("project-1", 3).unwrap();
    assert_eq!(session.document_snapshot().unwrap(), resized);
}

#[test]
fn batches_are_atomic_and_revisions_and_coordinates_are_checked() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Session::default();
    let p = s
        .create(CreateProject {
            width: 4,
            height: 4,
            palette: vec!["#FF0000FF".into(), "#00FF00FF".into()],
            output_path: dir.path().join("sprite.pixloom.json"),
        })
        .unwrap();
    let id = p["project_id"].as_str().unwrap();
    let op = |x, y| json!({"type":"set_pixels","frame_id":"frame-1","layer_id":"layer-1","pixels":[{"x":x,"y":y,"palette_index":0}]});
    assert!(
        s.apply(
            id,
            0,
            serde_json::from_value(json!([op(0, 0), op(4, 0)])).unwrap()
        )
        .is_err()
    );
    assert_eq!(s.preview(id, 1).unwrap().1["revision"], 0);
    assert!(
        s.apply(id, 1, serde_json::from_value(json!([op(0, 0)])).unwrap())
            .is_err()
    );
    assert!(
        s.apply(id, 0, serde_json::from_value(json!([op(-1, 0)])).unwrap())
            .is_err()
    );
    let result = s
        .apply(id, 0, serde_json::from_value(json!([op(0, 0)])).unwrap())
        .unwrap();
    assert_eq!(result["revision"], 1);
    assert_eq!(
        result["changed_bounds"],
        json!({"x":0,"y":0,"width":1,"height":1})
    );
    let recovery = s.recovery_path().unwrap().to_owned();
    assert!(recovery.exists());
    s.finish(id).unwrap();
    assert!(!recovery.exists());
    let doc = pixloom::schema::read_document(
        &std::fs::read(dir.path().join("sprite.pixloom.json")).unwrap(),
    )
    .unwrap();
    let pixels = pixloom::schema::decode_indices(&doc).unwrap();
    assert_eq!(pixels["cel-1"][0][0], 0);
    assert_ne!(pixels["cel-1"][0][1], 0);
}

#[test]
fn fill_is_four_connected_and_save_failure_keeps_session() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sprite.pixloom.json");
    let mut s = Session::default();
    let args = CreateProject {
        width: 3,
        height: 3,
        palette: vec!["#FF0000FF".into(), "#00FF00FF".into()],
        output_path: path.clone(),
    };
    let p = s.create(args.clone()).unwrap();
    let id = p["project_id"].as_str().unwrap();
    assert!(s.create(args).is_err());
    s.apply(id,0,serde_json::from_value(json!([
        {"type":"set_pixels","frame_id":"frame-1","layer_id":"layer-1","pixels":[{"x":1,"y":0,"palette_index":0},{"x":0,"y":1,"palette_index":0}]},
        {"type":"fill","frame_id":"frame-1","layer_id":"layer-1","x":0,"y":0,"palette_index":1}
    ])).unwrap()).unwrap();
    let recovery = s.recovery_path().unwrap();
    let doc = pixloom::schema::read_document(&std::fs::read(recovery).unwrap()).unwrap();
    let pixels = pixloom::schema::decode_indices(&doc).unwrap();
    assert_eq!(pixels["cel-1"][0][0], 1);
    assert_ne!(pixels["cel-1"][1][1], 1);
    std::fs::write(&path, b"keep me").unwrap();
    assert!(s.finish(id).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"keep me");
    assert_eq!(s.preview(id, 1).unwrap().1["revision"], 1);
    assert!(s.recovery_path().unwrap().exists());
}

#[test]
fn invalid_operations_leave_pixels_revision_and_recovery_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Session::default();
    let created = s
        .create(CreateProject {
            width: 3,
            height: 3,
            palette: vec!["#FFFFFFFF".into()],
            output_path: dir.path().join("a.pixloom.json"),
        })
        .unwrap();
    let id = created["project_id"].as_str().unwrap();
    let original = std::fs::read(s.recovery_path().unwrap()).unwrap();
    for invalid in [
        json!({"type":"line","x1":0,"y1":0,"x2":3,"y2":0,"palette_index":0}),
        json!({"type":"rectangle","x":0,"y":0,"width":0,"height":1,"filled":true,"palette_index":0}),
        json!({"type":"rectangle","x":2,"y":2,"width":2,"height":1,"filled":true,"palette_index":0}),
        json!({"type":"fill","x":0,"y":-1,"palette_index":0}),
        json!({"type":"clear","palette_index":99}),
        json!({"type":"clear","palette_index":0,"frame_id":"wrong"}),
    ] {
        let mut op = invalid;
        if op.get("frame_id").is_none() {
            op["frame_id"] = json!("frame-1");
        }
        op["layer_id"] = json!("layer-1");
        let valid =
            json!({"type":"clear","frame_id":"frame-1","layer_id":"layer-1","palette_index":0});
        assert!(
            s.apply(id, 0, serde_json::from_value(json!([valid, op])).unwrap())
                .is_err()
        );
        assert_eq!(std::fs::read(s.recovery_path().unwrap()).unwrap(), original);
        assert_eq!(s.preview(id, 1).unwrap().1["revision"], 0);
    }
    assert!(s.preview(id, 0).is_err());
    assert!(s.preview(id, u32::MAX).is_err());
    assert!(s.preview("wrong", 1).is_err());
}

#[test]
fn drawing_edges_clear_and_noop_fill() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Session::default();
    let p = s
        .create(CreateProject {
            width: 3,
            height: 3,
            palette: vec!["#FFFFFFFF".into(), "#000000FF".into()],
            output_path: dir.path().join("b.pixloom.json"),
        })
        .unwrap();
    let id = p["project_id"].as_str().unwrap();
    let ops = json!([
        {"type":"clear","frame_id":"frame-1","layer_id":"layer-1","palette_index":0},
        {"type":"rectangle","frame_id":"frame-1","layer_id":"layer-1","x":0,"y":0,"width":3,"height":3,"filled":false,"palette_index":1},
        {"type":"line","frame_id":"frame-1","layer_id":"layer-1","x1":2,"y1":0,"x2":0,"y2":2,"palette_index":0}
    ]);
    s.apply(id, 0, serde_json::from_value(ops).unwrap())
        .unwrap();
    let doc = pixloom::schema::read_document(&std::fs::read(s.recovery_path().unwrap()).unwrap())
        .unwrap();
    let indices = pixloom::schema::decode_indices(&doc).unwrap();
    assert_eq!(
        indices["cel-1"],
        vec![vec![1, 1, 0], vec![1, 0, 1], vec![0, 1, 1]]
    );
    let result=s.apply(id,1,serde_json::from_value(json!([{"type":"fill","frame_id":"frame-1","layer_id":"layer-1","x":1,"y":1,"palette_index":0}])).unwrap()).unwrap();
    assert_eq!(result["revision"], 2);
    assert!(result["changed_bounds"].is_null());
}
