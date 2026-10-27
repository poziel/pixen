use pixen::{
    preview::Crop,
    session::{CreateProject, Session},
};
use serde_json::{Value, json};
fn create(path: &std::path::Path) -> Session {
    let mut s = Session::default();
    s.create(CreateProject {
        width: 5,
        height: 5,
        palette: vec![
            "#00000000".into(),
            "#FF0000FF".into(),
            "#FF0000FF".into(),
            "#00FF00FF".into(),
        ],
        output_path: path.into(),
    })
    .unwrap();
    s
}
fn apply(s: &mut Session, rev: u64, ops: Value) -> Value {
    s.apply("project-1", rev, serde_json::from_value(ops).unwrap())
        .unwrap()
}
fn indices(s: &Session, frame: &str, layer: &str) -> Vec<u32> {
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
fn overlapping_moves_rotations_and_palette_remaps_preserve_indices_and_undo() {
    let d = tempfile::tempdir().unwrap();
    let mut s = create(&d.path().join("edit.pixloom"));
    apply(
        &mut s,
        0,
        json!([
            {"type":"set_pixels","frame_id":"frame-1","layer_id":"layer-1","pixels":[{"x":0,"y":0,"palette_index":1},{"x":1,"y":0,"palette_index":2},{"x":0,"y":1,"palette_index":3}]},
            {"type":"transform_region","frame_id":"frame-1","layer_id":"layer-1","region":{"x":0,"y":0,"width":2,"height":2},"x":1,"y":0,"transform":"rotate90"}
        ]),
    );
    let p = indices(&s, "frame-1", "layer-1");
    assert_eq!(&p[0..3], &[0, 3, 1]);
    assert_eq!(&p[5..8], &[0, 0, 2]);
    apply(
        &mut s,
        1,
        json!([{"type":"remap_color","from_index":1,"to_index":3,"layer_id":"layer-1"}]),
    );
    let p = indices(&s, "frame-1", "layer-1");
    assert_eq!(p[2], 3);
    assert_eq!(p[7], 2); // Equal RGBA index 2 is untouched.
    s.undo("project-1", 2).unwrap();
    assert_eq!(indices(&s, "frame-1", "layer-1")[2], 1);
    s.redo("project-1", 3).unwrap();
    assert_eq!(indices(&s, "frame-1", "layer-1")[2], 3);
    let bytes = std::fs::read(s.recovery_path().unwrap()).unwrap();
    let error=s.apply("project-1",4,serde_json::from_value(json!([
        {"type":"set_color","palette_index":1,"color":"#FFFFFFFF"},
        {"type":"copy_region","source_frame_id":"frame-1","source_layer_id":"layer-1","region":{"x":0,"y":0,"width":2,"height":1},"frame_id":"frame-1","layer_id":"layer-1","x":4,"y":0}
    ])).unwrap());
    assert!(error.is_err());
    assert_eq!(std::fs::read(s.recovery_path().unwrap()).unwrap(), bytes);
    assert_eq!(s.info("project-1").unwrap()["revision"], 4);
}
#[test]
fn project_checkpoint_reopen_and_external_conflicts_preserve_files_and_session() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("art.pixloom");
    let mut s = create(&path);
    s.save("project-1").unwrap();
    apply(
        &mut s,
        0,
        json!([{"type":"set_color","palette_index":1,"color":"#123456FF"}]),
    );
    s.finish("project-1").unwrap();
    let info = s.open(&path, None).unwrap();
    let id = info["project_id"].as_str().unwrap();
    assert_eq!(info["output_path"], json!(path.canonicalize().unwrap()));
    assert_eq!(info["palette"][1], "#123456FF");
    assert_eq!(info["palette"][2], "#FF0000FF");
    std::fs::write(&path, b"external change").unwrap();
    assert!(s.save(id).is_err());
    assert!(s.finish(id).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"external change");
    assert!(s.info(id).is_ok());
    assert!(s.recovery_path().unwrap().is_file());
}
#[test]
fn native_raster_and_json_open_as_editable_projects_with_pixen_default_destination() {
    let d = tempfile::tempdir().unwrap();
    let mut source = create(&d.path().join("original.pixloom"));
    apply(
        &mut source,
        0,
        json!([{"type":"rectangle","frame_id":"frame-1","layer_id":"layer-1","x":1,"y":1,"width":2,"height":2,"palette_index":3,"filled":true}]),
    );
    for extension in ["aseprite", "pxo", "png", "pixen.json"] {
        let path = d.path().join(format!("import-{extension}.{extension}"));
        source
            .export("project-1", &path, false, None, false)
            .unwrap();
        let original = std::fs::read(&path).unwrap();
        let mut s = Session::default();
        let info = s.open(&path, None).unwrap();
        let id = info["project_id"].as_str().unwrap();
        let dest = std::path::PathBuf::from(info["output_path"].as_str().unwrap());
        assert_eq!(dest.extension().unwrap(), "pixloom");
        assert_ne!(dest, path);
        assert!(info["frames"][0]["id"].as_str().is_some());
        s.apply(id,0,serde_json::from_value(json!([{"type":"set_pixels","frame_id":info["frames"][0]["id"],"layer_id":info["layers"][0]["id"],"pixels":[{"x":0,"y":0,"palette_index":0}]}])).unwrap()).unwrap();
        s.finish(id).unwrap();
        assert!(
            pixen::container::Reader::open(&dest)
                .unwrap()
                .load()
                .is_ok()
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
}
#[test]
fn read_only_analysis_counts_disappearing_pixels_and_isolates_hidden_layers() {
    let d = tempfile::tempdir().unwrap();
    let mut s = create(&d.path().join("analysis.pixloom"));
    apply(
        &mut s,
        0,
        json!([
            {"type":"add_layer","layer_id":"detail","name":"Detail"},
            {"type":"set_pixels","frame_id":"frame-1","layer_id":"detail","pixels":[{"x":1,"y":2,"palette_index":1}]},
            {"type":"update_layer","layer_id":"detail","visible":false},
            {"type":"add_frame","frame_id":"next","duration_ms":100}
        ]),
    );
    let (png, info) = s
        .analysis(
            "project-1",
            "next",
            Some("frame-1"),
            pixen::inspection::Mode::FrameComparison,
            2,
            Some("detail"),
        )
        .unwrap();
    assert_eq!(info["changed_pixels"], 1);
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(image.get_pixel(2, 4).0, [255, 64, 180, 255]);
    assert_eq!(image.dimensions(), (10, 10));
    let (_, info) = s
        .analysis(
            "project-1",
            "next",
            None,
            pixen::inspection::Mode::FrameComparison,
            1,
            None,
        )
        .unwrap();
    assert_eq!(info["changed_pixels"], 0);
    assert!(
        s.analysis(
            "project-1",
            "next",
            None,
            pixen::inspection::Mode::OnionSkin,
            0,
            None
        )
        .is_err()
    );
    assert_eq!(s.info("project-1").unwrap()["revision"], 1);
    assert_eq!(s.info("project-1").unwrap()["undo_steps"], 1);
}

#[test]
fn onion_skin_shows_neighbor_ghosts_keeps_current_pixels_and_does_not_wrap() {
    let d = tempfile::tempdir().unwrap();
    let mut s = create(&d.path().join("onion.pixloom"));
    apply(
        &mut s,
        0,
        json!([
            {"type":"set_pixels","frame_id":"frame-1","layer_id":"layer-1","pixels":[{"x":0,"y":0,"palette_index":1}]},
            {"type":"add_frame","frame_id":"middle","duration_ms":100},
            {"type":"set_pixels","frame_id":"middle","layer_id":"layer-1","pixels":[{"x":2,"y":0,"palette_index":3}]},
            {"type":"add_frame","frame_id":"last","duration_ms":100},
            {"type":"set_pixels","frame_id":"last","layer_id":"layer-1","pixels":[{"x":1,"y":0,"palette_index":1}]}
        ]),
    );
    let (png, _) = s
        .analysis(
            "project-1",
            "middle",
            None,
            pixen::inspection::Mode::OnionSkin,
            1,
            None,
        )
        .unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(image.get_pixel(0, 0).0, [255, 80, 80, 90]);
    assert_eq!(image.get_pixel(1, 0).0, [60, 210, 255, 90]);
    assert_eq!(image.get_pixel(2, 0).0, [0, 255, 0, 255]);
    let (png, _) = s
        .analysis(
            "project-1",
            "frame-1",
            None,
            pixen::inspection::Mode::OnionSkin,
            1,
            None,
        )
        .unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
    assert_eq!(image.get_pixel(1, 0).0, [0, 0, 0, 0]); // The last frame must not wrap behind the first.
    let (png, _) = s
        .analysis(
            "project-1",
            "middle",
            Some("last"),
            pixen::inspection::Mode::OnionSkin,
            1,
            None,
        )
        .unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(image.get_pixel(0, 0).0, [0, 0, 0, 0]);
    assert_eq!(image.get_pixel(1, 0).0, [255, 80, 80, 90]);
}
