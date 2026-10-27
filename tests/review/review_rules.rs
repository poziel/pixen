use pixen::{
    review::rules::ValidateArtworkRequest,
    session::{CreateProject, Session},
};
use serde_json::json;
#[test]
fn valid_rule_failures_are_reports_and_protected_regions_catch_palette_changes() {
    let d = tempfile::tempdir().unwrap();
    let mut s = Session::default();
    s.create(CreateProject {
        width: 2,
        height: 2,
        palette: vec!["#00000000".into(), "#FF0000FF".into()],
        output_path: d.path().join("a.pixloom"),
    })
    .unwrap();
    s.apply("project-1",0,serde_json::from_value(json!([{"type":"set_pixels","frame_id":"frame-1","layer_id":"layer-1","pixels":[{"x":0,"y":0,"palette_index":1}]}])).unwrap()).unwrap();
    let cp = s
        .manage_checkpoint(
            "project-1",
            serde_json::from_value(json!({"action":"create","expected_revision":1,"label":"face"}))
                .unwrap(),
        )
        .unwrap();
    let req:ValidateArtworkRequest=serde_json::from_value(json!({"rules":[{"type":"canvas_size","width":3,"height":2},{"type":"protected_region","checkpoint_id":cp["checkpoint_id"],"frame_ids":["frame-1"],"region":{"x":0,"y":0,"width":1,"height":1}}]})).unwrap();
    let r = s.validate_artwork("project-1", &req).unwrap();
    assert_eq!(r.report["passed"], false);
    assert_eq!(r.report["rules"][1]["passed"], true);
    s.apply(
        "project-1",
        1,
        serde_json::from_value(json!([{"type":"set_color","palette_index":1,"color":"#00FF00FF"}]))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        s.validate_artwork("project-1", &req).unwrap().report["rules"][1]["passed"],
        false
    );
    let bad =
        serde_json::from_value(json!({"rules":[{"type":"frame_count","min":2,"max":1}]})).unwrap();
    assert!(s.validate_artwork("project-1", &bad).is_err());
}
