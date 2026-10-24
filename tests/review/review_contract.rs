use pixloom::{
    review::{
        self,
        animation::AuditAnimationRequest,
        checkpoints::{CheckpointAction, CheckpointStore},
        compare::CompareCheckpointRequest,
        measure::AnalyzeSpriteRequest,
        rules::ValidateArtworkRequest,
    },
    schema,
};
use serde_json::json;
fn doc() -> schema::Document {
    schema::read_document(&serde_json::to_vec(&json!({"version":1,"canvas":{"width":2,"height":2},"palette":["#00000000","#FF0000FF","#FF0000FF"],"cels":[{"id":"c","pixels":[[1,0],[0,0]]}],"frames":[{"duration_ms":100,"annotations":{"metadata":{"pixloom_frame_id":"f"}}}],"layers":[{"id":"l","name":"Layer","timeline":[{"cel":"c"}]}]})).unwrap()).unwrap()
}
#[test]
fn checkpoint_capacity_is_explicit_and_failed_creation_does_not_reserve_ids() {
    let d = doc();
    let mut store = CheckpointStore::default();
    for i in 0..16 {
        store.create(&d, i, "same label").unwrap();
    }
    assert!(store.create(&d, 16, "overflow").is_err());
    assert_eq!(store.list()["checkpoints"].as_array().unwrap().len(), 16);
    store.delete("checkpoint-1").unwrap();
    assert_eq!(
        store.create(&d, 17, "next").unwrap()["checkpoint_id"],
        "checkpoint-17"
    );
    assert!(store.document("checkpoint-1").is_err());
}
#[test]
fn checkpoint_bytes_are_bounded_without_evicting_existing_baselines() {
    let mut d = doc();
    d.annotations.user_data = "a".repeat(9 * 1024 * 1024);
    let mut s = CheckpointStore::default();
    for i in 0..7 {
        s.create(&d, i, "large").unwrap();
    }
    assert!(s.create(&d, 7, "too much").is_err());
    assert_eq!(s.list()["checkpoints"].as_array().unwrap().len(), 7);
    assert!(s.document("checkpoint-1").is_ok());
}
#[test]
fn exact_index_and_rendered_changes_stay_separate_with_bounds() {
    let a = doc();
    let mut b = a.clone();
    let schema::CelDefinition::Legacy(c) = &mut b.cels[0] else {
        panic!()
    };
    c.pixels[0][0] = 2;
    let req: CompareCheckpointRequest =
        serde_json::from_value(json!({"checkpoint_id":"x","include_images":false})).unwrap();
    let r = review::compare::documents(&a, &b, &req).unwrap();
    assert_eq!(r.report["cells"][0]["index_changes"], 1);
    assert_eq!(r.report["cells"][0]["resolved_changes"], 0);
    assert_eq!(
        r.report["cells"][0]["changed_bounds"],
        json!({"x":0,"y":0,"width":1,"height":1})
    );
    assert_eq!(r.report["frames"][0]["diff"]["changed_pixels"], 0);
    let diff = review::compare::pixels(
        &[0, 0, 0, 0, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255],
        &[255, 0, 0, 255, 0, 0, 0, 0, 0, 255, 0, 255, 255, 0, 0, 127],
        4,
    );
    assert_eq!(
        (
            diff.added,
            diff.removed,
            diff.repainted,
            diff.alpha_only,
            diff.changed_pixels
        ),
        (1, 1, 1, 1, 4)
    );
}
#[test]
fn comparison_handles_canvas_changes_and_removed_targets() {
    let a = doc();
    let mut b = a.clone();
    b.canvas.width = 3;
    let schema::CelDefinition::Legacy(c) = &mut b.cels[0] else {
        panic!()
    };
    for row in &mut c.pixels {
        row.push(0);
    }
    b.layers[0].id = "new".into();
    let req = serde_json::from_value(json!({"checkpoint_id":"x","include_images":false})).unwrap();
    let r = review::compare::documents(&a, &b, &req).unwrap();
    assert_eq!(r.report["source"]["width"], 3);
    assert_eq!(r.report["added_layers"], json!(["new"]));
    assert_eq!(r.report["removed_layers"], json!(["l"]));
}
#[test]
fn measurement_truncates_evidence_but_keeps_full_component_totals() {
    let mut rgba = vec![0; 34 * 34 * 4];
    for y in (0..34).step_by(2) {
        for x in (0..34).step_by(2) {
            rgba[(y * 34 + x) * 4 + 3] = 255;
        }
    }
    let scope = review::render::ScopedRgba {
        source: pixloom::preview::Crop {
            x: 0,
            y: 0,
            width: 34,
            height: 34,
        },
        width: 34,
        height: 34,
        pixels: rgba,
    };
    let report = review::measure::pixels(&scope, 1, 8).unwrap();
    assert_eq!(report["component_count"], 289);
    assert_eq!(report["components"].as_array().unwrap().len(), 256);
    assert_eq!(report["truncation"]["components"], true);
    let image = review::measure::annotate(&scope, &report).unwrap();
    assert!(image::load_from_memory(&image.png).is_ok());
    assert!(review::measure::pixels(&scope, 0, 8).is_err());
    assert!(review::measure::pixels(&scope, 1, 6).is_err());
}
#[test]
fn raw_inventory_preserves_duplicate_indices_and_hidden_layer_inspection() {
    let mut d = doc();
    let schema::CelDefinition::Legacy(c) = &mut d.cels[0] else {
        panic!()
    };
    c.pixels[1][1] = 2;
    d.layers[0].visible = false;
    let req: AnalyzeSpriteRequest =
        serde_json::from_value(json!({"frame_id":"f","include_annotation":false})).unwrap();
    let r = review::measure::sprite(&d, &req).unwrap();
    assert_eq!(r.report["rendered_color_count"], 0);
    assert_eq!(r.report["palette_index_count"], 3);
    let mut req = req;
    req.layer_id = Some("l".into());
    let r = review::measure::sprite(&d, &req).unwrap();
    assert_eq!(r.report["rendered_color_count"], 1);
    assert_eq!(r.report["occupied_pixels"], 2);
    assert!(!d.layers[0].visible);
}
#[test]
fn rule_types_report_failures_and_reject_malformed_inputs() {
    let d = doc();
    let cps = CheckpointStore::default();
    let req:ValidateArtworkRequest=serde_json::from_value(json!({"rules":[{"type":"canvas_size","width":2,"height":2},{"type":"frame_count","min":1,"max":1},{"type":"frame_duration","min_ms":50,"max_ms":100},{"type":"allowed_palette_indices","indices":[0,1]},{"type":"max_rendered_colors","max_colors":1},{"type":"binary_alpha"}]})).unwrap();
    assert_eq!(
        review::rules::validate(&d, &cps, &req).unwrap().report["passed"],
        true
    );
    for rule in [
        json!({"type":"allowed_palette_indices","indices":[99]}),
        json!({"type":"frame_duration","min_ms":0,"max_ms":10}),
        json!({"type":"binary_alpha","scope":{"frame_ids":["missing"]}}),
    ] {
        let req = serde_json::from_value(json!({"rules":[rule]})).unwrap();
        assert!(review::rules::validate(&d, &cps, &req).is_err());
    }
    let many =
        serde_json::from_value(json!({"rules":vec![json!({"type":"binary_alpha"});65]})).unwrap();
    assert!(review::rules::validate(&d, &cps, &many).is_err());
    let empty = serde_json::from_value(json!({"rules":[]})).unwrap();
    assert!(review::rules::validate(&d, &cps, &empty).is_err());
    let mut partial = d.clone();
    partial.palette[1] = "#FF000080".into();
    let req = serde_json::from_value(json!({"rules":[{"type":"binary_alpha"}]})).unwrap();
    assert_eq!(
        review::rules::validate(&partial, &cps, &req)
            .unwrap()
            .report["passed"],
        false
    );
}
#[test]
fn protected_region_missing_targets_are_failures_not_argument_errors() {
    let mut a = doc();
    a.frames.push(a.frames[0].clone());
    a.frames[1]
        .annotations
        .metadata
        .insert("pixloom_frame_id".into(), json!("g"));
    let entry = a.layers[0].timeline[0].clone();
    a.layers[0].timeline.push(entry);
    let mut cps = CheckpointStore::default();
    cps.create(&a, 0, "initial").unwrap();
    let mut b = a.clone();
    b.frames.remove(0);
    b.layers[0].timeline.remove(0);
    let req=serde_json::from_value(json!({"rules":[{"type":"protected_region","checkpoint_id":"checkpoint-1","frame_ids":["f"],"region":{"x":0,"y":0,"width":1,"height":1}}]})).unwrap();
    let r = review::rules::validate(&b, &cps, &req).unwrap();
    assert_eq!(r.report["passed"], false);
    assert_eq!(r.report["rules"][0]["violation_count"], 1);
}
#[test]
fn animation_holds_and_subset_order_have_exact_timing_without_quality_scores() {
    let mut d = doc();
    for (id, duration) in [("g", 200.0), ("h", 300.0)] {
        let mut f = d.frames[0].clone();
        f.duration_ms = duration;
        f.annotations
            .metadata
            .insert("pixloom_frame_id".into(), json!(id));
        d.frames.push(f);
        let e = d.layers[0].timeline[0].clone();
        d.layers[0].timeline.push(e);
    }
    let req: AuditAnimationRequest =
        serde_json::from_value(json!({"include_loop_transition":true})).unwrap();
    let r = review::animation::audit(&d, &req).unwrap();
    assert_eq!(r.report["distinct_poses"], 1);
    assert_eq!(r.report["holds"][0]["duration_ms"], 600.0);
    assert_eq!(r.report["transitions"].as_array().unwrap().len(), 3);
    assert!(r.report.get("score").is_none());
    let req = serde_json::from_value(json!({"frame_ids":["h","f"]})).unwrap();
    let r = review::animation::audit(&d, &req).unwrap();
    assert_eq!(r.report["total_duration_ms"], 400.0);
    assert_eq!(r.report["transitions"][0]["from_frame_id"], "h");
    let bad = serde_json::from_value(json!({"frame_ids":["f","f"]})).unwrap();
    assert!(review::animation::audit(&d, &bad).is_err());
}
#[test]
fn checkpoint_management_is_read_only_and_stale_mutations_do_not_consume_ids() {
    let d = tempfile::tempdir().unwrap();
    let mut s = pixloom::session::Session::default();
    s.create(pixloom::session::CreateProject {
        width: 2,
        height: 2,
        palette: vec!["#00000000".into()],
        output_path: d.path().join("a.pixloom"),
    })
    .unwrap();
    let recovery = s.recovery_path().unwrap().to_owned();
    let before = std::fs::read(&recovery).unwrap();
    assert!(
        s.manage_checkpoint(
            "project-1",
            CheckpointAction::Create {
                expected_revision: 1,
                label: "bad".into()
            }
        )
        .is_err()
    );
    let r = s
        .manage_checkpoint(
            "project-1",
            CheckpointAction::Create {
                expected_revision: 0,
                label: "ok".into(),
            },
        )
        .unwrap();
    assert_eq!(r["checkpoint_id"], "checkpoint-1");
    assert_eq!(std::fs::read(&recovery).unwrap(), before);
    assert_eq!(s.info("project-1").unwrap()["undo_steps"], 0);
}

#[test]
fn fallback_frame_ids_do_not_alias_persisted_identifiers() {
    let mut d = doc();
    d.frames[0]
        .annotations
        .metadata
        .insert("pixloom_frame_id".into(), json!("frame-2"));
    let mut f = d.frames[0].clone();
    f.annotations.metadata.clear();
    d.frames.push(f);
    let e = d.layers[0].timeline[0].clone();
    d.layers[0].timeline.push(e);
    let ids = review::frame_ids(&d);
    assert_eq!(ids[0], "frame-2");
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn component_evidence_uses_integer_enlargement_for_legible_labels() {
    let d = doc();
    let req = serde_json::from_value(json!({"frame_id":"f"})).unwrap();
    let r = review::measure::sprite(&d, &req).unwrap();
    let img = image::load_from_memory(&r.images[0].png).unwrap();
    assert_eq!((img.width(), img.height()), (16, 16));
    assert_eq!(r.report["annotation_scale"], 8);
}

#[test]
fn malformed_review_requests_and_image_work_limits_fail_before_large_allocations() {
    let d = doc();
    for patch in [
        json!({"display_scale":0}),
        json!({"enlarged_scale":4294967295u32}),
        json!({"views":[]}),
        json!({"views":["color","color"]}),
        json!({"backgrounds":["#00000000"]}),
        json!({"backgrounds":vec!["#FFFFFFFF";5]}),
        json!({"crop":{"x":-1,"y":0,"width":1,"height":1}}),
    ] {
        let mut value = json!({"frame_id":"f"});
        for (k, v) in patch.as_object().unwrap() {
            value[k] = v.clone();
        }
        let req = serde_json::from_value(value).unwrap();
        assert!(review::render::sheet(&d, &req).is_err());
    }
    let mut big = d.clone();
    big.canvas.width = 4096;
    big.canvas.height = 1024;
    big.cels.clear();
    big.frames = (0..17)
        .map(|i| {
            let mut f = d.frames[0].clone();
            f.annotations
                .metadata
                .insert("pixloom_frame_id".into(), json!(format!("f{i}")));
            f
        })
        .collect();
    big.layers[0].timeline = vec![
        schema::Entry {
            cel: None,
            link: None,
            opacity: 1.0,
            annotations: Default::default()
        };
        17
    ];
    schema::validate_document(&big).unwrap();
    let req: AuditAnimationRequest = serde_json::from_value(json!({})).unwrap();
    assert!(
        review::animation::audit(&big, &req)
            .unwrap_err()
            .to_string()
            .contains("64 million")
    );
}
#[test]
fn rendered_color_inventory_and_empty_motion_keep_honest_totals() {
    let mut rgba = Vec::new();
    for y in 0..18 {
        for x in 0..18 {
            rgba.extend_from_slice(&[x, y, 0, 255]);
        }
    }
    let scope = review::render::ScopedRgba {
        source: pixloom::preview::Crop {
            x: 0,
            y: 0,
            width: 18,
            height: 18,
        },
        width: 18,
        height: 18,
        pixels: rgba,
    };
    let r = review::measure::pixels(&scope, 1, 8).unwrap();
    assert_eq!(r["rendered_color_count"], 324);
    assert_eq!(r["rendered_colors"].as_array().unwrap().len(), 256);
    assert_eq!(r["truncation"]["rendered_colors"], true);
    let mut d = doc();
    let mut f = d.frames[0].clone();
    f.annotations
        .metadata
        .insert("pixloom_frame_id".into(), json!("g"));
    d.frames.push(f);
    let mut e = d.layers[0].timeline[0].clone();
    e.cel = None;
    d.layers[0].timeline.push(e);
    let req = serde_json::from_value(json!({"max_centroid_displacement":0})).unwrap();
    let r = review::animation::audit(&d, &req).unwrap();
    assert!(r.report["transitions"][0]["centroid_displacement"].is_null());
    assert!(r.report["transitions"][0]["occupied_area_ratio"].is_null());
    assert_eq!(r.report["transitions"][0]["empty_transition"], true);
    assert!(r.report["findings"].as_array().unwrap().is_empty());
}
