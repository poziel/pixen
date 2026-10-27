use pixen::{
    preview::Crop,
    session::{CreateProject, Session},
};
use serde_json::{Value, json};

fn create(dir: &std::path::Path) -> Session {
    let mut s = Session::default();
    s.create(CreateProject {
        width: 5,
        height: 5,
        palette: vec!["#00000000".into(), "#FF0000FF".into(), "#00FF0080".into()],
        output_path: dir.join("edit.pixen.json"),
    })
    .unwrap();
    s
}
#[test]
fn compact_pixel_rows_preserve_surroundings_and_reject_invalid_batches_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = create(dir.path());
    apply(&mut s, 0, json!([{"type":"set_pixel_rows","frame_id":"frame-1","layer_id":"layer-1","x":1,"y":2,"rows":[[1,2],[2,1]]}]));
    let before = pixels(&s, "frame-1", "layer-1");
    assert_eq!(&before[11..13], &[1,2]);
    assert_eq!(&before[16..18], &[2,1]);
    assert_eq!(before[0], 0);
    for (x, y, rows) in [
        (0,0,json!([])), (0,0,json!([[]])), (0,0,json!([[1],[1,2]])),
        (4,4,json!([[1,2]])), (-1,0,json!([[1]])),
        (i64::MAX,0,json!([[1,2]])), (0,i64::MAX,json!([[1],[2]])),
        (0,0,json!([[1,999]])),
    ] {
        let ops = serde_json::from_value(json!([
            {"type":"clear","frame_id":"frame-1","layer_id":"layer-1","palette_index":2},
            {"type":"set_pixel_rows","frame_id":"frame-1","layer_id":"layer-1","x":x,"y":y,"rows":rows}
        ])).unwrap();
        assert!(s.apply("project-1", 1, ops).is_err());
        assert_eq!(pixels(&s,"frame-1","layer-1"), before);
    }
    s.undo("project-1", 1).unwrap();
    assert!(pixels(&s,"frame-1","layer-1").iter().all(|i| *i == 0));
}
#[test]
fn clipboard_paste_imports_colors_replaces_whole_cels_and_detaches_destinations() {
    let dir=tempfile::tempdir().unwrap();let mut s=create(dir.path());
    apply(&mut s,0,json!([
        {"type":"add_frame","frame_id":"second","duration_ms":100},
        {"type":"link_cells","layer_id":"layer-1","source_frame_id":"frame-1","frame_ids":["second"],"link_id":"linked"},
        {"type":"clear","layer_id":"layer-1","frame_id":"frame-1","palette_index":1}
    ]));
    let mut rgba=vec![[0,0,0,0];25];rgba[0]=[0,0,255,128];
    apply(&mut s,1,json!([{"type":"paste_cells","layer_id":"layer-1","frame_ids":["second"],"width":5,"height":5,"pixels":rgba,"opacity":0.5}]));
    assert_eq!(pixels(&s,"frame-1","layer-1")[0],1);
    assert_eq!(pixels(&s,"second","layer-1")[1],0);
    assert_ne!(pixels(&s,"second","layer-1")[0],1);
    let doc=s.current_document().unwrap();
    assert_eq!(doc.layers[0].timeline[1].opacity,0.5);
    assert!(doc.layers[0].timeline[1].link.is_none());
    assert_eq!(doc.palette[pixels(&s,"second","layer-1")[0] as usize],"#0000FF80");
    s.undo("project-1",2).unwrap();assert_eq!(pixels(&s,"second","layer-1")[1],1);
}
fn apply(s: &mut Session, rev: u64, ops: Value) -> Value {
    s.apply("project-1", rev, serde_json::from_value(ops).unwrap())
        .unwrap()
}
fn pixels(s: &Session, frame: &str, layer: &str) -> Vec<u32> {
    s.pixels(
        "project-1",
        frame,
        layer,
        Crop {
            x: 0,
            y: 0,
            width: 5,
            height: 5,
        },
    )
    .unwrap()["pixels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["palette_index"].as_u64().unwrap() as u32)
        .collect()
}
#[test]
fn matrix_cells_link_empty_artwork_unlink_copy_and_undo() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = create(dir.path());
    apply(&mut s, 0, json!([
        {"type":"add_frame","frame_id":"second","duration_ms":100},
        {"type":"add_frame","frame_id":"third","duration_ms":100},
        {"type":"link_cells","layer_id":"layer-1","source_frame_id":"frame-1","frame_ids":["second","third"],"link_id":"shared"},
        {"type":"set_pixels","layer_id":"layer-1","frame_id":"second","pixels":[{"x":0,"y":0,"palette_index":1}]}
    ]));
    for frame in ["frame-1", "second", "third"] { assert_eq!(pixels(&s, frame, "layer-1")[0], 1); }
    apply(&mut s, 1, json!([
        {"type":"unlink_cells","layer_id":"layer-1","frame_ids":["second"]},
        {"type":"set_pixels","layer_id":"layer-1","frame_id":"second","pixels":[{"x":0,"y":0,"palette_index":2}]}
    ]));
    assert_eq!(pixels(&s, "second", "layer-1")[0], 2);
    assert_eq!(pixels(&s, "frame-1", "layer-1")[0], 1);
    apply(&mut s, 2, json!([
        {"type":"copy_cells","layer_id":"layer-1","source_frame_id":"second","frame_ids":["third"]},
        {"type":"set_pixels","layer_id":"layer-1","frame_id":"third","pixels":[{"x":0,"y":0,"palette_index":0}]}
    ]));
    assert_eq!(pixels(&s, "third", "layer-1")[0], 0);
    assert_eq!(pixels(&s, "second", "layer-1")[0], 2);
    s.undo("project-1", 3).unwrap();
    assert_eq!(pixels(&s, "third", "layer-1")[0], 1);
    s.undo("project-1", 4).unwrap();
    apply(&mut s, 5, json!([{"type":"set_pixels","layer_id":"layer-1","frame_id":"second","pixels":[{"x":1,"y":0,"palette_index":2}]}]));
    for frame in ["frame-1", "second", "third"] { assert_eq!(pixels(&s, frame, "layer-1")[1], 2); }
}

#[test]
fn matrix_invalid_target_rolls_back_and_link_ids_cannot_merge_accidentally() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = create(dir.path());
    apply(&mut s, 0, json!([
        {"type":"add_frame","frame_id":"second","duration_ms":100},
        {"type":"link_cells","layer_id":"layer-1","source_frame_id":"frame-1","frame_ids":["second"],"link_id":"shared"}
    ]));
    for operation in [
        json!({"type":"unlink_cells","layer_id":"layer-1","frame_ids":["second","missing"]}),
        json!({"type":"link_cells","layer_id":"layer-1","source_frame_id":"second","frame_ids":["frame-1"],"link_id":"shared"}),
    ] {
        assert!(s.apply("project-1", 1, serde_json::from_value(json!([operation])).unwrap()).is_err());
        assert_eq!(s.info("project-1").unwrap()["revision"], 1);
    }
    apply(&mut s, 1, json!([{"type":"set_pixels","layer_id":"layer-1","frame_id":"second","pixels":[{"x":0,"y":0,"palette_index":1}]}]));
    assert_eq!(pixels(&s, "frame-1", "layer-1")[0], 1);
}

#[test]
fn clearing_sparse_cells_detaches_only_selected_artwork_and_is_atomic_and_undoable() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = create(dir.path());
    apply(&mut s, 0, json!([
        {"type":"add_frame","frame_id":"second","duration_ms":100},
        {"type":"add_layer","layer_id":"ink","name":"Ink"},
        {"type":"link_cells","layer_id":"layer-1","source_frame_id":"frame-1","frame_ids":["second"],"link_id":"shared"},
        {"type":"set_pixels","layer_id":"layer-1","frame_id":"frame-1","pixels":[{"x":0,"y":0,"palette_index":1}]},
        {"type":"set_pixels","layer_id":"ink","frame_id":"second","pixels":[{"x":0,"y":0,"palette_index":2}]}
    ]));
    apply(&mut s, 1, json!([
        {"type":"clear_cells","layer_id":"layer-1","frame_ids":["second"]},
        {"type":"clear_cells","layer_id":"ink","frame_ids":["second"]}
    ]));
    assert_eq!(pixels(&s, "second", "layer-1")[0], 0);
    assert_eq!(pixels(&s, "frame-1", "layer-1")[0], 1);
    assert_eq!(pixels(&s, "second", "ink")[0], 0);
    s.undo("project-1", 2).unwrap();
    assert_eq!(pixels(&s, "second", "layer-1")[0], 1);
    assert_eq!(pixels(&s, "second", "ink")[0], 2);
    assert!(s.apply("project-1", 3, serde_json::from_value(json!([
        {"type":"clear_cells","layer_id":"ink","frame_ids":["second"]},
        {"type":"clear_cells","layer_id":"layer-1","frame_ids":["missing"]}
    ])).unwrap()).is_err());
    assert_eq!(pixels(&s, "second", "ink")[0], 2);
}
#[test]
fn mixed_transactions_ordering_duplication_and_history_are_independent() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = create(dir.path());
    apply(
        &mut s,
        0,
        json!([
            {"type":"add_layer","layer_id":"ink","name":"Ink"},
            {"type":"rectangle","frame_id":"frame-1","layer_id":"ink","x":1,"y":1,"width":3,"height":3,"filled":true,"palette_index":1},
            {"type":"duplicate_frame","frame_id":"frame-1","new_frame_id":"second"},
            {"type":"erase_region","frame_id":"second","layer_id":"ink","x":1,"y":1,"width":1,"height":1}
        ]),
    );
    assert_eq!(pixels(&s, "frame-1", "ink")[6], 1);
    assert_eq!(pixels(&s, "second", "ink")[6], 0);
    let info = s.info("project-1").unwrap();
    assert_eq!(info["revision"], 1);
    assert_eq!(info["frames"][1]["id"], "second");
    apply(
        &mut s,
        1,
        json!([
            {"type":"reorder_frame","frame_id":"second","index":0},
            {"type":"set_frame_duration","frame_id":"second","duration_ms":250.0},
            {"type":"reorder_layer","layer_id":"ink","index":0},
            {"type":"rename_layer","layer_id":"ink","name":"Contours"},
            {"type":"update_layer","layer_id":"ink","visible":false,"opacity":0.5}
        ]),
    );
    let info = s.info("project-1").unwrap();
    assert_eq!(info["frames"][0]["id"], "second");
    assert_eq!(info["frames"][0]["duration_ms"], 250.0);
    assert_eq!(info["layers"][0]["id"], "ink");
    assert_eq!(info["layers"][0]["name"], "Contours");
    assert_eq!(
        s.inspect("project-1", "frame-1", None, 1).unwrap().1["frame_id"],
        "frame-1"
    );
    assert_eq!(s.undo("project-1", 2).unwrap()["revision"], 3);
    assert_eq!(s.info("project-1").unwrap()["frames"][0]["id"], "frame-1");
    assert_eq!(s.undo("project-1", 3).unwrap()["revision"], 4);
    assert_eq!(
        s.info("project-1").unwrap()["layers"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(s.redo("project-1", 4).unwrap()["revision"], 5);
    assert_eq!(pixels(&s, "frame-1", "ink")[6], 1);
    apply(
        &mut s,
        5,
        json!([{"type":"clear_cel","frame_id":"second","layer_id":"ink"}]),
    );
    assert!(s.redo("project-1", 6).is_err());
    assert_eq!(pixels(&s, "frame-1", "ink")[6], 1);
}

#[test]
fn invalid_batches_and_stale_history_never_change_document_or_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = create(dir.path());
    let before = std::fs::read(s.recovery_path().unwrap()).unwrap();
    for invalid in [
        json!({"type":"rectangle","frame_id":"frame-1","layer_id":"layer-1","x":4,"y":0,"width":2,"height":1,"filled":true,"palette_index":1}),
        json!({"type":"remove_frame","frame_id":"frame-1"}),
        json!({"type":"remove_layer","layer_id":"layer-1"}),
        json!({"type":"update_layer","layer_id":"layer-1","opacity":1.1}),
        json!({"type":"set_frame_duration","frame_id":"frame-1","duration_ms":0.0}),
    ] {
        assert!(s.apply("project-1",0,serde_json::from_value(json!([{"type":"clear","frame_id":"frame-1","layer_id":"layer-1","palette_index":1},invalid])).unwrap()).is_err());
        assert_eq!(std::fs::read(s.recovery_path().unwrap()).unwrap(), before);
        assert!(s.undo("project-1", 0).is_err());
    }
    apply(
        &mut s,
        0,
        json!([{"type":"add_frame","frame_id":"blank","duration_ms":75.0}]),
    );
    assert!(s.undo("project-1", 0).is_err());
    assert_eq!(s.info("project-1").unwrap()["revision"], 1);
    assert_eq!(pixels(&s, "blank", "layer-1"), vec![0; 25]);
    apply(
        &mut s,
        1,
        json!([{"type":"remove_frame","frame_id":"blank"}]),
    );
    s.undo("project-1", 2).unwrap();
    assert_eq!(pixels(&s, "blank", "layer-1"), vec![0; 25]);
}

#[test]
fn ellipse_geometry_erase_and_exact_region_semantics() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = create(dir.path());
    apply(
        &mut s,
        0,
        json!([{"type":"ellipse","frame_id":"frame-1","layer_id":"layer-1","x":0,"y":0,"width":5,"height":5,"filled":true,"palette_index":1}]),
    );
    assert_eq!(
        pixels(&s, "frame-1", "layer-1"),
        vec![
            0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 0
        ]
    );
    apply(
        &mut s,
        1,
        json!([
            {"type":"clear_cel","frame_id":"frame-1","layer_id":"layer-1"},
            {"type":"ellipse","frame_id":"frame-1","layer_id":"layer-1","x":0,"y":0,"width":5,"height":5,"filled":false,"palette_index":2}
        ]),
    );
    assert_eq!(pixels(&s, "frame-1", "layer-1")[12], 0);
    let region = s
        .pixels(
            "project-1",
            "frame-1",
            "layer-1",
            Crop {
                x: 1,
                y: 0,
                width: 1,
                height: 1,
            },
        )
        .unwrap();
    assert_eq!(
        region["pixels"][0],
        json!({"x":1,"y":0,"palette_index":2,"rgba":"#00FF0080"})
    );
    apply(
        &mut s,
        2,
        json!([{"type":"erase_region","frame_id":"frame-1","layer_id":"layer-1","x":0,"y":0,"width":5,"height":1}]),
    );
    assert!(
        pixels(&s, "frame-1", "layer-1")[..5]
            .iter()
            .all(|&p| p == 0)
    );
    assert!(
        s.pixels(
            "project-1",
            "frame-1",
            "layer-1",
            Crop {
                x: -1,
                y: 0,
                width: 1,
                height: 1
            }
        )
        .is_err()
    );
}

#[test]
fn explicit_links_share_edits_but_unlinked_reuse_and_duplicates_detach() {
    let dir = tempfile::tempdir().unwrap();
    let s = create(dir.path());
    let mut doc =
        pixen::schema::read_document(&std::fs::read(s.recovery_path().unwrap()).unwrap()).unwrap();
    doc.frames = vec![doc.frames[0].clone(); 3];
    for (i, f) in doc.frames.iter_mut().enumerate() {
        f.annotations.metadata.insert(
            pixen::edit::FRAME_ID_KEY.into(),
            json!(format!("frame-{}", i + 1)),
        );
    }
    let mut entry = doc.layers[0].timeline[0].clone();
    entry.link = Some("shared".into());
    doc.layers[0].timeline = vec![
        entry.clone(),
        entry.clone(),
        doc.layers[0].timeline[0].clone(),
    ];
    let mut other = doc.layers[0].clone();
    other.id = "other".into();
    for e in &mut other.timeline {
        e.link = None;
    }
    doc.layers.push(other);
    let mut used = std::collections::BTreeSet::from([
        "frame:frame-1".into(),
        "frame:frame-2".into(),
        "frame:frame-3".into(),
        "layer:layer-1".into(),
        "layer:other".into(),
    ]);
    let mut next = 1;
    let edit=serde_json::from_value(json!({"type":"set_pixels","frame_id":"frame-1","layer_id":"layer-1","pixels":[{"x":0,"y":0,"palette_index":1}]})).unwrap();
    pixen::edit::execute(&mut doc, edit, &mut used, &mut next).unwrap();
    pixen::schema::validate_document(&doc).unwrap();
    assert_eq!(pixen::edit::grid(&doc, 0, 0).unwrap()[0][0], 1);
    assert_eq!(pixen::edit::grid(&doc, 0, 1).unwrap()[0][0], 1);
    assert_eq!(pixen::edit::grid(&doc, 0, 2).unwrap()[0][0], 0);
    assert_eq!(pixen::edit::grid(&doc, 1, 0).unwrap()[0][0], 0);
    let edit = serde_json::from_value(
        json!({"type":"duplicate_frame","frame_id":"frame-1","new_frame_id":"copy"}),
    )
    .unwrap();
    pixen::edit::execute(&mut doc, edit, &mut used, &mut next).unwrap();
    let edit =
        serde_json::from_value(json!({"type":"clear_cel","frame_id":"copy","layer_id":"layer-1"}))
            .unwrap();
    pixen::edit::execute(&mut doc, edit, &mut used, &mut next).unwrap();
    pixen::schema::validate_document(&doc).unwrap();
    assert_eq!(pixen::edit::grid(&doc, 0, 0).unwrap()[0][0], 1);
    assert_eq!(pixen::edit::grid(&doc, 0, 1).unwrap()[0][0], 0);
    assert_eq!(doc.layers[0].timeline[0].link.as_deref(), Some("shared"));
    assert!(doc.layers[0].timeline[1].link.is_none());
}

#[test]
fn history_is_bounded_and_failed_recovery_write_does_not_consume_a_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = create(dir.path());
    for revision in 0..35 {
        apply(
            &mut s,
            revision,
            json!([{"type":"clear","frame_id":"frame-1","layer_id":"layer-1","palette_index":revision%2}]),
        );
    }
    assert_eq!(s.info("project-1").unwrap()["undo_steps"], 32);
    let path = s.recovery_path().unwrap().to_owned();
    let saved = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(s.undo("project-1", 35).is_err());
    assert_eq!(s.info("project-1").unwrap()["revision"], 35);
    assert_eq!(s.info("project-1").unwrap()["undo_steps"], 32);
    std::fs::remove_dir(&path).unwrap();
    std::fs::write(&path, saved).unwrap();
    for revision in 35..67 {
        s.undo("project-1", revision).unwrap();
    }
    assert!(s.undo("project-1", 67).is_err());
    assert_eq!(s.info("project-1").unwrap()["redo_steps"], 32);
}

#[test]
fn preview_selection_tracks_identity_and_never_changes_explicit_edit_targets() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = create(dir.path());
    apply(
        &mut s,
        0,
        json!([{"type":"duplicate_frame","frame_id":"frame-1","new_frame_id":"copy"}]),
    );
    let before =
        pixen::schema::read_document(&std::fs::read(s.recovery_path().unwrap()).unwrap()).unwrap();
    let mut playback = pixen::preview::Playback::new(2);
    playback.select(1, 2);
    apply(
        &mut s,
        1,
        json!([{"type":"reorder_frame","frame_id":"copy","index":0}]),
    );
    let after =
        pixen::schema::read_document(&std::fs::read(s.recovery_path().unwrap()).unwrap()).unwrap();
    playback.reconcile_frames(&before.frames, &after.frames);
    assert_eq!(playback.frame, 0);
    assert!(!playback.playing);
    apply(
        &mut s,
        2,
        json!([{"type":"set_pixels","frame_id":"frame-1","layer_id":"layer-1","pixels":[{"x":0,"y":0,"palette_index":1}]}]),
    );
    assert_eq!(pixels(&s, "copy", "layer-1")[0], 0);
    assert_eq!(pixels(&s, "frame-1", "layer-1")[0], 1);
}
