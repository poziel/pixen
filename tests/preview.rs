use pixloom::preview::{Crop, Playback, load, png};
use std::time::Duration;

#[test]
fn supported_project_preview_is_read_only_and_crops_match_renderer() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sample.pixloom.json");
    let original = include_bytes!("../skills/pixloom/examples/animated.pixloom.json");
    std::fs::write(&path, original).unwrap();
    let before = std::fs::metadata(&path).unwrap().modified().unwrap();
    let sprite = load(&path).unwrap();
    let crop = Crop {
        x: 1,
        y: 1,
        width: 2,
        height: 2,
    };
    let (bytes, source) = png(&sprite, 1, Some(crop), 2).unwrap();
    assert_eq!(source, crop);
    let image = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (4, 4));
    let composite = pixloom::raster::render(&sprite, 1).unwrap();
    for y in 0..4 {
        for x in 0..4 {
            let index = (((y / 2 + 1) * sprite.width + x / 2 + 1) * 4) as usize;
            assert_eq!(&image.get_pixel(x, y).0, &composite[index..index + 4]);
        }
    }
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        before
    );
    assert!(png(&sprite, 0, Some(Crop { x: -1, ..crop }), 1).is_err());
    assert!(png(&sprite, 0, Some(Crop { width: 0, ..crop }), 1).is_err());
    assert!(png(&sprite, 0, Some(Crop { x: 3, ..crop }), 1).is_err());
    assert!(png(&sprite, 999, None, 1).is_err());
    assert!(png(&sprite, 0, None, 0).is_err());
    assert!(load(&dir.path().join("missing.pixloom.json")).is_err());
    std::fs::write(dir.path().join("unsupported.txt"), b"hello").unwrap();
    assert!(load(&dir.path().join("unsupported.txt")).is_err());
}

#[test]
fn playback_obeys_durations_loops_selection_stop_and_frame_changes() {
    let mut p = Playback::new(3);
    let durations = [100.0, 200.0, 50.0];
    p.advance(Duration::from_millis(99), &durations);
    assert_eq!(p.frame, 0);
    p.advance(Duration::from_millis(1), &durations);
    assert_eq!(p.frame, 1);
    p.advance(Duration::from_millis(199), &durations);
    assert_eq!(p.frame, 1);
    p.advance(Duration::from_millis(51), &durations);
    assert_eq!(p.frame, 0);
    p.select(2, 3);
    assert!(!p.playing);
    p.advance(Duration::from_secs(10), &durations);
    assert_eq!(p.frame, 2);
    p.reconcile(1);
    assert_eq!(p.frame, 0);
    assert!(!p.playing);
    p.reconcile(4);
    p.toggle(4);
    p.advance(Duration::from_secs(1000), &[100.0; 4]);
    assert_eq!(p.frame, 0);
    p.stop();
    assert_eq!(p.frame, 0);
    assert!(!p.playing);
    p.select(999, 4);
    assert_eq!(p.frame, 3);
    p.toggle(4);
    p.reconcile(2);
    assert_eq!(p.frame, 1);
    assert!(p.playing);
    p.advance(Duration::from_millis(200), &[100.0, 200.0]);
    assert_eq!(p.frame, 0);
    p.reconcile(5);
    assert!(p.playing);
    p.advance(Duration::from_millis(100), &[100.0; 5]);
    assert_eq!(p.frame, 1);
}

#[test]
fn native_and_container_projects_use_the_same_composite_without_writes() {
    let dir = tempfile::tempdir().unwrap();
    for relative in [
        "tests/fixtures/pixelorama-1.2.3-static.pxo",
        "tests/fixtures/aseprite/basic-16x16.aseprite",
    ] {
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        let copy = dir.path().join(source.file_name().unwrap());
        let original = std::fs::read(&source).unwrap();
        std::fs::write(&copy, &original).unwrap();
        let sprite = load(&copy).unwrap();
        let expected = pixloom::raster::render(&sprite, 0).unwrap();
        let (bytes, _) = png(&sprite, 0, None, 1).unwrap();
        assert_eq!(
            image::load_from_memory(&bytes).unwrap().to_rgba8().as_raw(),
            &expected
        );
        assert_eq!(std::fs::read(&copy).unwrap(), original);
    }
    let source = dir.path().join("container.pixloom.json");
    std::fs::write(
        &source,
        include_bytes!("../skills/pixloom/examples/animated.pixloom.json"),
    )
    .unwrap();
    let packed = dir.path().join("container.pixloom");
    pixloom::container::pack(&source, &packed, false, false).unwrap();
    let original = std::fs::read(&packed).unwrap();
    let sprite = load(&packed).unwrap();
    assert_eq!(sprite.frames.len(), 4);
    assert_eq!(
        pixloom::raster::render(&sprite, 2).unwrap(),
        pixloom::raster::render(&load(&source).unwrap(), 2).unwrap()
    );
    assert_eq!(std::fs::read(&packed).unwrap(), original);
}
