use pixloom::desktop::{Application, DocumentWorkspace, RenderRequest};

#[test]
fn tabs_preserve_independent_unsaved_documents_and_save_targets() {
    let root = tempfile::tempdir().unwrap();
    let mut workspace = DocumentWorkspace::new(Application::new(false));
    workspace.create(4, 3).unwrap();
    let first = workspace.snapshot().unwrap().active_document.unwrap();
    workspace.active().resize(7, 5).unwrap();
    workspace.create(32, 32).unwrap();
    let second = workspace.snapshot().unwrap().active_document.unwrap();
    assert!(!workspace.snapshot().unwrap().active.document.unwrap().dirty);
    workspace.active().resize(33, 32).unwrap();
    assert_ne!(first, second);
    assert_eq!(workspace.snapshot().unwrap().documents.len(), 2);
    workspace.activate(&first).unwrap();
    let doc = workspace.snapshot().unwrap().active.document.unwrap();
    assert_eq!((doc.width, doc.height), (7, 5));
    assert!(doc.dirty);
    let path = root.path().join("first.pixloom");
    workspace.active().save(Some(&path)).unwrap();
    workspace.activate(&second).unwrap();
    let state = workspace.snapshot().unwrap();
    assert_eq!(state.active.document.unwrap().width, 33);
    assert!(
        state
            .documents
            .iter()
            .find(|doc| doc.id == second)
            .unwrap()
            .dirty
    );
    assert!(
        !state
            .documents
            .iter()
            .find(|doc| doc.id == first)
            .unwrap()
            .dirty
    );
    workspace.close(&first, false).unwrap();
    assert!(workspace.close(&second, false).is_err());
    assert_eq!(workspace.snapshot().unwrap().documents.len(), 1);
    workspace.close(&second, true).unwrap();
    let empty = workspace.snapshot().unwrap();
    assert!(empty.documents.is_empty());
    assert!(empty.active.document.is_none());
}

#[test]
fn failed_opens_are_atomic_and_reopening_a_file_activates_its_existing_tab() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("saved.pixloom");
    let mut workspace = DocumentWorkspace::new(Application::new(false));
    workspace.create(4, 3).unwrap();
    workspace.active().save(Some(&path)).unwrap();
    let first = workspace.snapshot().unwrap().active_document.unwrap();
    workspace.create(2, 2).unwrap();
    let second = workspace.snapshot().unwrap().active_document.unwrap();
    assert!(workspace.open(&root.path().join("missing.png")).is_err());
    assert_eq!(
        workspace.snapshot().unwrap().active_document.unwrap(),
        second
    );
    workspace.open(&path).unwrap();
    let state = workspace.snapshot().unwrap();
    assert_eq!(state.active_document.unwrap(), first);
    assert_eq!(state.documents.len(), 2);
    workspace.open(&path).unwrap();
    assert_eq!(workspace.snapshot().unwrap().documents.len(), 2);
}

#[test]
fn document_identity_rejects_a_late_render_from_another_tab() {
    let mut workspace = DocumentWorkspace::new(Application::new(false));
    workspace.create(2, 2).unwrap();
    let first = workspace.snapshot().unwrap();
    let doc = first.active.document.unwrap();
    let request = RenderRequest {
        generation: doc.generation,
        frame_id: doc.frames[0].id.clone(),
        ..Default::default()
    };
    workspace.create(2, 2).unwrap();
    assert!(
        workspace
            .render(first.active_document.as_deref(), request)
            .is_err()
    );
}

#[test]
fn managed_preview_cannot_open_or_create_extra_tabs() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source.pixloom");
    let mut writer = Application::new(false);
    writer.create(2, 2).unwrap();
    writer.save(Some(&path)).unwrap();
    let mut app = Application::new(true);
    app.open(&path).unwrap();
    let mut workspace = DocumentWorkspace::new(app);
    let id = workspace.snapshot().unwrap().active_document.unwrap();
    assert!(workspace.create(3, 3).is_err());
    assert!(workspace.open(&path).is_err());
    assert!(workspace.close(&id, true).is_err());
    assert!(workspace.activate("another").is_err());
    assert_eq!(workspace.snapshot().unwrap().documents.len(), 1);
}

#[test]
fn untouched_new_canvas_closes_without_discard_and_edits_become_dirty_until_saved() {
    let root = tempfile::tempdir().unwrap();
    let mut workspace = DocumentWorkspace::new(Application::new(false));
    workspace.create(32, 32).unwrap();
    let snapshot = workspace.snapshot().unwrap();
    assert!(!snapshot.active.document.unwrap().dirty);
    workspace
        .close(snapshot.active_document.as_deref().unwrap(), false)
        .unwrap();
    workspace.create(16, 16).unwrap();
    workspace.active().resize(17, 16).unwrap();
    assert!(workspace.snapshot().unwrap().active.document.unwrap().dirty);
    workspace
        .active()
        .save(Some(&root.path().join("saved.pixloom")))
        .unwrap();
    assert!(!workspace.snapshot().unwrap().active.document.unwrap().dirty);
}
