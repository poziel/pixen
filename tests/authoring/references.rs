use pixen::{pxo, raster, schema};
use std::{io::Read, path::Path};

#[test]
#[ignore = "requires original animation PXO in PIXEN_ANIMATION_FIXTURE"]
fn original_animation_preserves_all_frames_links_and_native_swatches() {
    let path = std::env::var_os("PIXEN_ANIMATION_FIXTURE").expect("set PIXEN_ANIMATION_FIXTURE");
    let s = pxo::read(Path::new(&path)).unwrap();
    assert_eq!(
        (s.width, s.height, s.layers.len(), s.frames.len()),
        (640, 400, 17, 128)
    );
    assert_eq!(s.fps, 8.0);
    assert!(s.frames.iter().all(|f| f.duration_ms == 125.0));
    assert_eq!((s.tags[0].from, s.tags[0].to), (1, 128));
    assert_eq!(s.pixelorama.palettes.len(), 1);
    assert_eq!(
        s.pixelorama.project_current_palette_name,
        "DawnBringer 32 copy"
    );
    assert_eq!(
        s.layers
            .iter()
            .map(|l| l
                .timeline
                .iter()
                .filter_map(|e| e.link.as_ref())
                .collect::<std::collections::HashSet<_>>()
                .len())
            .sum::<usize>(),
        140
    );
    let unpacked = schema::to_document(&s).unwrap();
    let rebuilt = schema::compile(unpacked).unwrap();
    for f in [0, 23, 48, 99, 127] {
        assert_eq!(
            raster::render(&s, f).unwrap(),
            raster::render(&rebuilt, f).unwrap()
        );
    }
    let mut file = tempfile::NamedTempFile::new().unwrap();
    pxo::write(&mut file, &rebuilt).unwrap();
    let r = pxo::read(file.path()).unwrap();
    assert_eq!(r.pixelorama.palettes, s.pixelorama.palettes);
    for f in [0, 23, 48, 99, 127] {
        assert_eq!(
            raster::render(&s, f).unwrap(),
            raster::render(&r, f).unwrap()
        );
    }
    for (a, b) in s.layers.iter().zip(&r.layers) {
        for (a, b) in a.timeline.iter().zip(&b.timeline) {
            assert_eq!(a.link, b.link);
            assert_eq!(
                s.images[a.cel.as_ref().unwrap()],
                r.images[b.cel.as_ref().unwrap()]
            );
        }
    }
}

#[test]
fn target_pixelorama_exports_generated_animation_with_correct_pixels_timing_tags_and_links() {
    let dir = std::env::var_os("PIXEN_NATIVE_DIR").unwrap_or_else(|| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/native-animation"
        )
        .into()
    });
    let dir = Path::new(&dir);
    let doc = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/skills/pixen/examples/animated.json"
    ))
    .unwrap();
    let s = schema::parse(&doc).unwrap();
    let file = std::fs::File::open(dir.join("animated.png")).unwrap();
    let mut reader = png::Decoder::new(file).read_info().unwrap();
    assert_eq!((reader.info().width, reader.info().height), (16, 4));
    let mut buf = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut buf).unwrap();
    for f in 0..4 {
        let rgba = raster::render(&s, f).unwrap();
        for row in 0..4 {
            let a = (row * 16 + f * 4) * 4;
            assert_eq!(&buf[a..a + 16], &rgba[row * 16..row * 16 + 16]);
        }
    }
    let meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("animated.json")).unwrap()).unwrap();
    assert_eq!(meta["pixelorama_version"], "v1.2.3-stable");
    assert_eq!(meta["frames"][1]["duration"], 2.0);
    assert_eq!(
        meta["layers"][0]["link_sets"][0]["cels"],
        serde_json::json!([0, 2])
    );
    assert_eq!(meta["tags"][0]["from"], 1);
    assert_eq!(meta["tags"][0]["to"], 4);
}

#[test]
fn malformed_zip_lengths_are_rejected() {
    let mut original = Vec::new();
    std::fs::File::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/pixelorama-1.2.3-static.pxo"
    ))
    .unwrap()
    .read_to_end(&mut original)
    .unwrap();
    // Mutate a central-directory length, independently of our ZIP writer.
    let pos = original
        .windows(4)
        .position(|b| b == b"PK\x01\x02")
        .unwrap();
    original[pos + 24..pos + 28].copy_from_slice(&u32::MAX.to_le_bytes());
    let f = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(f.path(), &original).unwrap();
    assert!(pixen::archive::Archive::open(f.path()).is_err());
}

#[test]
fn independent_native_fixture_confirms_layer_order_visibility_and_combined_opacity() {
    let source = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/authored-layers.json"
    ))
    .unwrap();
    let s = schema::parse(&source).unwrap();
    let file = std::fs::File::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/pixelorama-1.2.3-layers.png"
    ))
    .unwrap();
    let mut reader = png::Decoder::new(file).read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (2, 1));
    assert_eq!(pixels, [223, 0, 32, 255, 0, 0, 255, 32]);
    assert_eq!(raster::render(&s, 0).unwrap(), pixels);
}

#[test]
#[ignore = "requires independent Pixelorama PNG in PIXEN_NATIVE_REFERENCE_PNG"]
fn original_reference_frame_is_within_one_level_of_native_headless_export() {
    let input = std::env::var_os("PIXEN_ANIMATION_FIXTURE").expect("set PIXEN_ANIMATION_FIXTURE");
    let export =
        std::env::var_os("PIXEN_NATIVE_REFERENCE_PNG").expect("set PIXEN_NATIVE_REFERENCE_PNG");
    let s = pxo::read(Path::new(&input)).unwrap();
    let rendered = raster::render(&s, 23).unwrap();
    let mut decoder = png::Decoder::new(std::fs::File::open(export).unwrap());
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (640, 400));
    assert_eq!(info.color_type, png::ColorType::Rgba);
    let different = rendered.iter().zip(&pixels).filter(|(a, b)| a != b).count();
    let max_delta = rendered
        .iter()
        .zip(&pixels)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    println!("native reference comparison: {different} differing channels, max delta {max_delta}");
    // Pixelorama quantizes intermediate RGBA8 blends; pixen rounds once.
    assert!(max_delta <= 1);
    assert!(
        rendered
            .as_chunks::<4>()
            .0
            .iter()
            .zip(pixels.as_chunks::<4>().0)
            .all(|(a, b)| a[3] == b[3])
    );
}
