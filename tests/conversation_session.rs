use pixloom::session::{CreateProject, Session};
use serde_json::json;

#[test]
fn turn_restore_and_save_as_preserve_session_and_original() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("original.pixloom");
    let mut session = Session::default();
    let created = session
        .create(CreateProject {
            width: 2,
            height: 2,
            palette: vec!["#00000000".into(), "#FFFFFFFF".into()],
            output_path: original.clone(),
        })
        .unwrap();
    let id = created["project_id"].as_str().unwrap();
    session.save(id).unwrap();
    let original_bytes = std::fs::read(&original).unwrap();
    let baseline = session.document_snapshot().unwrap();
    let operations = serde_json::from_value(json!([{"type":"set_pixels", "frame_id":"frame-1", "layer_id":"layer-1", "pixels":[{"x":0,"y":0,"palette_index":1}]}])).unwrap();
    session.apply(id, 0, operations).unwrap();
    session.restore_document(&baseline).unwrap();
    assert_eq!(session.info(id).unwrap()["revision"], 2);
    let destination = dir.path().join("saved.pixloom");
    session.save_as(id, &destination).unwrap();
    assert_eq!(
        session.info(id).unwrap()["output_path"],
        json!(destination.canonicalize().unwrap())
    );
    assert_eq!(std::fs::read(original).unwrap(), original_bytes);
    assert!(
        session
            .save_as(id, &dir.path().join("saved.pixloom"))
            .is_ok()
    );
    assert_eq!(session.active_id(), Some(id));
}

#[test]
fn recovery_failure_rejects_edit_without_advancing_revision() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = Session::default();
    let created = session
        .create(CreateProject {
            width: 1,
            height: 1,
            palette: vec!["#00000000".into(), "#FFFFFFFF".into()],
            output_path: dir.path().join("art.pixloom"),
        })
        .unwrap();
    let id = created["project_id"].as_str().unwrap();
    let recovery = session.recovery_path().unwrap().to_owned();
    std::fs::remove_file(&recovery).unwrap();
    std::fs::create_dir(&recovery).unwrap();
    let operations = serde_json::from_value(json!([{"type":"set_pixels", "frame_id":"frame-1", "layer_id":"layer-1", "pixels":[{"x":0,"y":0,"palette_index":1}]}])).unwrap();
    assert!(session.apply(id, 0, operations).is_err());
    let info = session.info(id).unwrap();
    assert_eq!(info["revision"], 0);
    assert_eq!(info["undo_steps"], 0);
}

#[test]
fn save_as_collision_preserves_destination_and_current_save_path() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("art.pixloom");
    let existing = dir.path().join("existing.pixloom");
    std::fs::write(&existing, b"unrelated content").unwrap();
    let mut session = Session::default();
    let created = session
        .create(CreateProject {
            width: 1,
            height: 1,
            palette: vec!["#00000000".into()],
            output_path: output.clone(),
        })
        .unwrap();
    let id = created["project_id"].as_str().unwrap();
    assert!(session.save_as(id, &existing).is_err());
    assert_eq!(std::fs::read(existing).unwrap(), b"unrelated content");
    assert_eq!(
        session.info(id).unwrap()["output_path"],
        json!(pixloom::output::resolved(&output).unwrap())
    );
}
