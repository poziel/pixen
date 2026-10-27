use pixen::{edit, preview, raster};
use std::{collections::HashSet, io::Cursor};

#[test]
fn delivered_mcp_example_is_distinct_layered_and_preserves_gif_frames_and_timing() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/mcp-animation/output");
    let project = preview::load(&root.join("bounce.pixloom.json")).unwrap();
    let gif_path = root.join("bounce.gif");
    let original = std::fs::read(&gif_path).unwrap();
    let modified = std::fs::metadata(&gif_path).unwrap().modified().unwrap();
    let gif = preview::load(&gif_path).unwrap();
    let ids = ["frame-1", "rise", "apex-in", "apex-out", "fall", "land"];
    let durations = [200.0, 100.0, 140.0, 220.0, 90.0, 120.0];
    assert_eq!((project.width, project.height), (24, 24));
    assert_eq!(
        project
            .layers
            .iter()
            .map(|l| l.id.as_str())
            .collect::<Vec<_>>(),
        ["ground", "layer-1"]
    );
    assert_eq!(project.frames.len(), ids.len());
    assert_eq!(gif.frames.len(), ids.len());
    let mut distinct = HashSet::new();
    for (i, (&id, duration)) in ids.iter().zip(durations).enumerate() {
        assert_eq!(edit::frame_id(&project.frames[i]), Some(id));
        assert_eq!(project.frames[i].duration_ms, duration);
        assert_eq!(gif.frames[i].duration_ms, duration);
        let pixels = raster::render(&project, i).unwrap();
        assert_eq!(raster::render(&gif, i).unwrap(), pixels);
        distinct.insert(pixels);
    }
    assert_eq!(
        distinct.len(),
        6,
        "animation requires six visually distinct composites"
    );
    let mut decoder = gif::DecodeOptions::new()
        .read_info(Cursor::new(&original))
        .unwrap();
    while decoder.read_next_frame().unwrap().is_some() {}
    assert_eq!(decoder.repeat(), gif::Repeat::Infinite);
    assert_eq!(std::fs::read(&gif_path).unwrap(), original);
    assert_eq!(
        std::fs::metadata(&gif_path).unwrap().modified().unwrap(),
        modified
    );
}
