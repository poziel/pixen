use pixloom::{
    review::{
        Representation,
        render::{ReviewSheetRequest, sheet, transform},
    },
    schema,
};
use serde_json::json;
#[test]
fn representations_preserve_alpha_and_sheet_keeps_source_coordinates() {
    assert_eq!(
        transform(&[255, 0, 0, 128], Representation::Grayscale),
        vec![54, 54, 54, 128]
    );
    assert_eq!(
        transform(&[255, 0, 0, 128], Representation::Silhouette),
        vec![0, 0, 0, 128]
    );
    let doc=schema::read_document(&serde_json::to_vec(&json!({"version":1,"canvas":{"width":2,"height":1},"palette":["#00000000","#FF000080"],"cels":[{"id":"c","pixels":[[0,1]]}],"frames":[{"duration_ms":100,"annotations":{"metadata":{"pixloom_frame_id":"f"}}}],"layers":[{"id":"l","name":"secret helmet","timeline":[{"cel":"c"}]}]})).unwrap()).unwrap();
    let mut doc = doc;
    doc.frames[0]
        .annotations
        .metadata
        .insert("pixloom_frame_id".into(), json!("f"));
    let req:ReviewSheetRequest=serde_json::from_value(json!({"frame_id":"f","display_scale":1,"enlarged_scale":2,"views":["color","silhouette"],"crop":{"x":1,"y":0,"width":1,"height":1}})).unwrap();
    let result = sheet(&doc, &req).unwrap();
    assert_eq!(result.report["source"]["x"], 1);
    assert_eq!(result.report["panels"].as_array().unwrap().len(), 8);
    assert_eq!(result.images.len(), 1);
    let mut bad = req.clone();
    bad.enlarged_scale = u32::MAX;
    assert!(sheet(&doc, &bad).is_err());
}
