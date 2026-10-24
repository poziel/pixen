use pixloom::{
    review::checkpoints::CheckpointAction,
    session::{CreateProject, Session},
};
use serde_json::json;
#[test]
fn checkpoint_survives_history_and_detects_palette_only_changes() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Session::default();
    s.create(CreateProject {
        width: 2,
        height: 2,
        palette: vec!["#00000000".into(), "#FF0000FF".into()],
        output_path: dir.path().join("a.pixloom"),
    })
    .unwrap();
    s.apply("project-1",0,serde_json::from_value(json!([{"type":"set_pixels","frame_id":"frame-1","layer_id":"layer-1","pixels":[{"x":0,"y":0,"palette_index":1}]}])).unwrap()).unwrap();
    let cp = s
        .manage_checkpoint(
            "project-1",
            serde_json::from_value::<CheckpointAction>(
                json!({"action":"create","expected_revision":1,"label":"blockout"}),
            )
            .unwrap(),
        )
        .unwrap();
    let cid = cp["checkpoint_id"].as_str().unwrap();
    s.apply(
        "project-1",
        1,
        serde_json::from_value(json!([{"type":"set_color","palette_index":1,"color":"#00FF00FF"}]))
            .unwrap(),
    )
    .unwrap();
    let req = serde_json::from_value(json!({"checkpoint_id":cid,"include_images":false})).unwrap();
    let diff = s.compare_checkpoint("project-1", &req).unwrap();
    assert_eq!(diff.report["frames"][0]["diff"]["repainted"], 1);
    assert_eq!(diff.report["cells"][0]["index_changes"], 0);
    assert_eq!(diff.report["cells"][0]["resolved_changes"], 1);
    s.undo("project-1", 2).unwrap();
    let diff = s.compare_checkpoint("project-1", &req).unwrap();
    assert_eq!(diff.report["frames"][0]["diff"]["changed_pixels"], 0);
    s.save("project-1").unwrap();
    assert_eq!(s.info("project-1").unwrap()["revision"], 3);
}

#[test]
fn equivalent_hex_spelling_does_not_report_resolved_color_changes() {
    let mut a = pixloom::schema::read_document(include_bytes!(
        "../../skills/pixloom/examples/animated.pixloom.json"
    ))
    .unwrap();
    for (i, f) in a.frames.iter_mut().enumerate() {
        f.annotations
            .metadata
            .insert("pixloom_frame_id".into(), json!(format!("f{i}")));
    }
    for color in &mut a.palette {
        *color = color.to_uppercase();
    }
    let mut b = a.clone();
    for color in &mut b.palette {
        *color = color.to_lowercase();
    }
    let request =
        serde_json::from_value(json!({"checkpoint_id":"unused","include_images":false})).unwrap();
    let r = pixloom::review::compare::documents(&a, &b, &request).unwrap();
    assert!(
        r.report["cells"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["resolved_changes"] == 0)
    );
}
