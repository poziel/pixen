use pixloom::{
    review::animation::{AuditAnimationRequest, audit},
    schema,
};
use serde_json::json;
#[test]
fn audit_reports_holds_timing_and_loop_as_observations() {
    let mut doc = schema::read_document(include_bytes!(
        "../../skills/pixloom/examples/animated.pixloom.json"
    ))
    .unwrap();
    for (i, f) in doc.frames.iter_mut().enumerate() {
        f.annotations
            .metadata
            .insert("pixloom_frame_id".into(), json!(format!("f{i}")));
    }
    let req: AuditAnimationRequest = serde_json::from_value(
        json!({"frame_ids":["f0","f1"],"include_loop_transition":true,"scale":1}),
    )
    .unwrap();
    let r = audit(&doc, &req).unwrap();
    assert_eq!(r.report["transitions"].as_array().unwrap().len(), 2);
    assert_eq!(
        r.report["total_duration_ms"],
        doc.frames[0].duration_ms + doc.frames[1].duration_ms
    );
    assert_eq!(r.images.len(), 1);
    assert!(
        r.report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["severity"] != "failure")
    );
}
