use pixen::preview::Crop;
use pixen::review::{measure, render::ScopedRgba};
#[test]
fn geometry_components_thresholds_and_truncation_are_exact() {
    let scope = ScopedRgba {
        source: Crop {
            x: 5,
            y: 7,
            width: 2,
            height: 2,
        },
        width: 2,
        height: 2,
        pixels: vec![255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 0, 127],
    };
    let a = measure::pixels(&scope, 1, 4).unwrap();
    assert_eq!(a["component_count"], 2);
    assert_eq!(a["bounds"]["x"], 5);
    assert_eq!(a["centroid"]["x"], 5.5);
    assert_eq!(measure::pixels(&scope, 1, 8).unwrap()["component_count"], 1);
    assert_eq!(
        measure::pixels(&scope, 128, 8).unwrap()["occupied_pixels"],
        1
    );
    let empty = ScopedRgba {
        pixels: vec![0; 16],
        ..scope
    };
    let e = measure::pixels(&empty, 1, 8).unwrap();
    assert!(e["centroid"].is_null());
    assert!(e["padding"].is_null());
}
