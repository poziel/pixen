use image::{Rgba, RgbaImage};
use pixloom::import::{self, Decoded, Options, TransparentMode};
use std::{path::Path, process::Command};
fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pixloom"))
        .args(args)
        .output()
        .unwrap()
}
fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}
fn decoded(images: Vec<RgbaImage>) -> Decoded {
    let durations = vec![100.; images.len()];
    Decoded {
        images,
        durations,
        annotations: Default::default(),
        warnings: vec![],
    }
}
fn raw(doc: pixloom::schema::Document) -> Vec<Vec<u8>> {
    let sprite = pixloom::schema::compile(doc).unwrap();
    sprite.layers[0]
        .timeline
        .iter()
        .map(|e| sprite.images[e.cel.as_ref().unwrap()].to_vec())
        .collect()
}
#[test]
fn static_formats_exact_pixels_determinism_and_workflows() {
    let dir = tempfile::tempdir().unwrap();
    let image = RgbaImage::from_raw(
        2,
        2,
        vec![1, 2, 3, 0, 4, 5, 6, 127, 7, 8, 9, 255, 1, 2, 3, 0],
    )
    .unwrap();
    let input = dir.path().join("source image.png");
    image.save(&input).unwrap();
    let mut docs = vec![];
    for (i, ext) in ["pixloom.json", "pixloom"].into_iter().enumerate() {
        let output = dir.path().join(format!("arbitrary name.{ext}"));
        let r = cli(&["import", s(&input), "--output", s(&output)]);
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
        let doc = pixloom::input::read(&output).unwrap();
        assert_eq!(raw(doc.clone()), vec![image.as_raw().clone()]);
        assert_eq!(doc.layers[0].name, "Artwork");
        assert_eq!(doc.palette.len(), 3);
        docs.push(serde_json::to_value(doc).unwrap());
        for command in ["validate", "build", "export"] {
            let base = dir.path().join(format!("workflow{i}{command}"));
            let mut args = vec![command, s(&output)];
            if command != "validate" {
                args.extend(["--output", s(&base)]);
            }
            if command == "build" {
                args.extend(["--formats", "pxo,aseprite"]);
            }
            let r = cli(&args);
            assert!(
                r.status.success(),
                "{command}: {}",
                String::from_utf8_lossy(&r.stderr)
            );
        }
        let bytes = std::fs::read(&output).unwrap();
        assert!(
            !cli(&["import", s(&input), "--output", s(&output)])
                .status
                .success()
        );
        assert_eq!(bytes, std::fs::read(&output).unwrap());
        assert!(
            cli(&["import", s(&input), "--output", s(&output), "--force"])
                .status
                .success()
        );
        assert_eq!(bytes, std::fs::read(&output).unwrap());
    }
    assert_eq!(docs[0], docs[1]);
    let json = dir.path().join("arbitrary name.pixloom.json");
    let packed = dir.path().join("packed.pixloom");
    assert!(
        cli(&["pack", s(&json), "--output", s(&packed)])
            .status
            .success()
    );
    assert_eq!(
        docs[0],
        serde_json::to_value(pixloom::input::read(&packed).unwrap()).unwrap()
    );
}
#[test]
fn jpeg_matches_decoded_pixels_and_lossless_webp() {
    let dir = tempfile::tempdir().unwrap();
    let img = image::RgbImage::from_fn(8, 8, |x, y| {
        image::Rgb([(x * 30) as u8, (y * 30) as u8, 80])
    });
    for ext in ["jpg", "webp"] {
        let path = dir.path().join(format!("input.{ext}"));
        img.save(&path).unwrap();
        let expected = image::open(&path).unwrap().to_rgba8().into_raw();
        assert_eq!(
            raw(import::document(import::decode(&path).unwrap()).unwrap()),
            vec![expected]
        );
    }
}
#[test]
fn tolerance_border_connectivity_and_enclosed_details() {
    let white = Rgba([255, 255, 255, 255]);
    let black = Rgba([0, 0, 0, 255]);
    let mut img = RgbaImage::from_pixel(5, 5, white);
    for x in 1..4 {
        img.put_pixel(x, 1, black);
        img.put_pixel(x, 3, black);
    }
    img.put_pixel(1, 2, black);
    img.put_pixel(3, 2, black);
    img.put_pixel(0, 0, Rgba([240, 250, 245, 123]));
    let mut d = decoded(vec![img.clone()]);
    let mut opts = Options {
        transparent_color: Some([255; 3]),
        color_tolerance: 15,
        transparent_mode: TransparentMode::Border,
        trim: false,
    };
    import::transform(&mut d, &opts);
    assert_eq!(d.images[0].get_pixel(0, 0).0, [240, 250, 245, 0]);
    assert_eq!(*d.images[0].get_pixel(2, 2), white);
    opts.transparent_mode = TransparentMode::All;
    opts.color_tolerance = 0;
    let mut d = decoded(vec![img]);
    import::transform(&mut d, &opts);
    assert_eq!(d.images[0].get_pixel(2, 2)[3], 0);
    assert_eq!(d.images[0].get_pixel(0, 0)[3], 123);
    // Transparent pixels connect matching regions to the boundary, but diagonal contact does not.
    let mut img = RgbaImage::from_pixel(3, 3, black);
    img.put_pixel(1, 0, Rgba([7, 8, 9, 0]));
    img.put_pixel(1, 1, white);
    let mut d = decoded(vec![img]);
    opts.transparent_mode = TransparentMode::Border;
    import::transform(&mut d, &opts);
    assert_eq!(d.images[0].get_pixel(1, 1)[3], 0);
}
#[test]
fn shared_trim_preserves_alignment_pixels_and_empty_canvas() {
    let mut a = RgbaImage::new(6, 5);
    let mut b = a.clone();
    a.put_pixel(1, 2, Rgba([1, 2, 3, 128]));
    b.put_pixel(4, 3, Rgba([4, 5, 6, 255]));
    let mut d = decoded(vec![a, b]);
    import::transform(
        &mut d,
        &Options {
            trim: true,
            ..Default::default()
        },
    );
    assert_eq!(d.images[0].dimensions(), (4, 2));
    assert_eq!(d.images[1].dimensions(), (4, 2));
    assert_eq!(d.images[0].get_pixel(0, 0).0, [1, 2, 3, 128]);
    assert_eq!(d.images[1].get_pixel(3, 1).0, [4, 5, 6, 255]);
    let mut d = decoded(vec![RgbaImage::new(6, 5)]);
    import::transform(
        &mut d,
        &Options {
            trim: true,
            ..Default::default()
        },
    );
    assert_eq!(d.images[0].dimensions(), (6, 5));
}
#[test]
fn failures_preserve_input_destination_and_leave_no_temporary_files() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.pixloom");
    RgbaImage::new(1, 1)
        .save_with_format(&input, image::ImageFormat::Png)
        .unwrap();
    let bytes = std::fs::read(&input).unwrap();
    assert!(import::run(&input, &input, &Options::default(), true).is_err());
    assert_eq!(bytes, std::fs::read(&input).unwrap());
    let bad = dir.path().join("bad.png");
    std::fs::write(&bad, b"\x89PNG\r\n\x1a\ntruncated").unwrap();
    let dest = dir.path().join("result.pixloom.json");
    std::fs::write(&dest, b"retain me").unwrap();
    assert!(import::run(&bad, &dest, &Options::default(), true).is_err());
    assert_eq!(std::fs::read(&dest).unwrap(), b"retain me");
    assert!(
        import::run(
            &input,
            &dir.path().join("bad.json"),
            &Options::default(),
            false
        )
        .is_err()
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 3);
    for args in [
        vec!["--color-tolerance", "1"],
        vec!["--transparent-mode", "border"],
        vec!["--transparent-color", "#fff"],
        vec!["--transparent-color", "#ffffff", "--color-tolerance", "256"],
    ] {
        let mut all = vec!["import", s(&input), "--output", s(&dest)];
        all.extend(args);
        assert!(!cli(&all).status.success());
    }
}
#[test]
fn palette_limit_fails_without_quantization() {
    let img = RgbaImage::from_fn(1025, 1024, |x, y| {
        let n = y * 1025 + x;
        Rgba([(n >> 16) as u8, (n >> 8) as u8, n as u8, 255])
    });
    assert!(
        import::document(decoded(vec![img]))
            .unwrap_err()
            .to_string()
            .contains("palette")
    );
}
#[test]
fn oversized_header_and_high_precision_fail_before_pixel_decode() {
    let dir = tempfile::tempdir().unwrap();
    for (name, width, depth) in [
        ("huge", 4097, png::BitDepth::Eight),
        ("precise", 1, png::BitDepth::Sixteen),
    ] {
        let path = dir.path().join(format!("{name}.png"));
        let file = std::fs::File::create(&path).unwrap();
        let mut encoder = png::Encoder::new(file, width, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(depth);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&vec![
                0;
                width as usize
                    * 4
                    * if depth == png::BitDepth::Sixteen {
                        2
                    } else {
                        1
                    }
            ])
            .unwrap();
        writer.finish().unwrap();
        let error = import::decode(&path).err().unwrap().to_string();
        assert!(
            error.contains(if name == "huge" { "canvas" } else { "8 bits" }),
            "{error}"
        );
    }
}
#[test]
fn gif_offsets_disposal_durations_and_shared_palette() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("animation.gif");
    {
        let mut encoder = gif::Encoder::new(
            std::fs::File::create(&path).unwrap(),
            3,
            1,
            &[0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255],
        )
        .unwrap();
        encoder.set_repeat(gif::Repeat::Finite(2)).unwrap();
        for (left, index, delay, dispose) in [
            (0, 1, 3, gif::DisposalMethod::Keep),
            (1, 2, 7, gif::DisposalMethod::Previous),
            (2, 3, 11, gif::DisposalMethod::Background),
            (1, 2, 13, gif::DisposalMethod::Keep),
        ] {
            let frame = gif::Frame {
                left,
                width: 1,
                height: 1,
                delay,
                dispose,
                buffer: std::borrow::Cow::Owned(vec![index]),
                ..Default::default()
            };
            encoder.write_frame(&frame).unwrap();
        }
    }
    let d = import::decode(&path).unwrap();
    assert_eq!(d.durations, [30., 70., 110., 130.]);
    assert_eq!(d.images.len(), 4);
    let r = [255, 0, 0, 255];
    let g = [0, 255, 0, 255];
    let b = [0, 0, 255, 255];
    let t = [0, 0, 0, 0];
    for (img, expected) in d
        .images
        .iter()
        .zip([[r, t, t], [r, g, t], [r, t, b], [r, g, t]])
    {
        assert_eq!(img.as_raw(), &expected.concat());
    }
    assert!(
        d.annotations
            .metadata
            .contains_key("raster_import_loop_count")
    );
    let doc = import::document(d).unwrap();
    assert_eq!(doc.palette.len(), 4);
}
#[test]
fn apng_blending_offsets_previous_disposal_and_fractional_timing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("animation.png");
    {
        let mut encoder = png::Encoder::new(std::fs::File::create(&path).unwrap(), 3, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_animated(3, 2).unwrap();
        encoder.set_frame_delay(1, 3).unwrap();
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&[255, 0, 0, 255, 255, 0, 0, 255, 0, 0, 0, 0])
            .unwrap();
        writer.set_frame_dimension(1, 1).unwrap();
        writer.set_frame_position(1, 0).unwrap();
        writer.set_blend_op(png::BlendOp::Over).unwrap();
        writer.set_dispose_op(png::DisposeOp::Previous).unwrap();
        writer.set_frame_delay(1, 10).unwrap();
        writer.write_image_data(&[0, 255, 0, 128]).unwrap();
        writer.set_frame_position(2, 0).unwrap();
        writer.set_blend_op(png::BlendOp::Source).unwrap();
        writer.set_dispose_op(png::DisposeOp::Background).unwrap();
        writer.set_frame_delay(2, 10).unwrap();
        writer.write_image_data(&[0, 0, 255, 255]).unwrap();
        writer.finish().unwrap();
    }
    let d = import::decode(&path).unwrap();
    assert_eq!(d.images.len(), 3);
    assert!((d.durations[0] - 1000. / 3.).abs() < 1e-10);
    assert_eq!(d.durations[1..], [100., 200.]);
    assert_eq!(d.images[1].get_pixel(1, 0).0, [127, 128, 0, 255]);
    assert_eq!(d.images[2].get_pixel(1, 0).0, [255, 0, 0, 255]);
    assert_eq!(d.images[2].get_pixel(2, 0).0, [0, 0, 255, 255]);
}
fn chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut v = tag.to_vec();
    v.extend((data.len() as u32).to_le_bytes());
    v.extend(data);
    if !data.len().is_multiple_of(2) {
        v.push(0);
    }
    v
}
fn u24(n: u32) -> [u8; 3] {
    [n as u8, (n >> 8) as u8, (n >> 16) as u8]
}
#[test]
fn animated_webp_offsets_disposal_and_timing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("animation.webp");
    let mut payload = b"WEBP".to_vec();
    let mut header = vec![0x12, 0, 0, 0];
    header.extend(u24(3));
    header.extend(u24(0));
    payload.extend(chunk(b"VP8X", &header));
    payload.extend(chunk(b"ANIM", &[0, 0, 0, 0, 2, 0]));
    for (x, color, delay, flags) in [
        (0, [255, 0, 0, 255], 30, 2),
        (0, [0, 255, 0, 128], 70, 1),
        (2, [0, 0, 255, 255], 90, 2),
    ] {
        let img = RgbaImage::from_pixel(2, 1, Rgba(color));
        let mut encoded = Cursor::new(vec![]);
        img.write_to(&mut encoded, image::ImageFormat::WebP)
            .unwrap();
        let bytes = encoded.into_inner();
        let mut frame = vec![];
        frame.extend(u24(x / 2));
        frame.extend(u24(0));
        frame.extend(u24(1));
        frame.extend(u24(0));
        frame.extend(u24(delay));
        frame.push(flags);
        frame.extend(&bytes[12..]);
        payload.extend(chunk(b"ANMF", &frame));
    }
    let mut bytes = b"RIFF".to_vec();
    bytes.extend((payload.len() as u32).to_le_bytes());
    bytes.extend(payload);
    std::fs::write(&path, bytes).unwrap();
    let d = import::decode(&path).unwrap();
    assert_eq!(d.images.len(), 3);
    assert_eq!(d.durations, [30., 70., 90.]);
    assert_eq!(d.images[0].get_pixel(0, 0).0, [255, 0, 0, 255]);
    assert_eq!(d.images[1].get_pixel(0, 0).0, [127, 128, 0, 255]);
    assert_eq!(d.images[2].get_pixel(0, 0)[3], 0);
    assert_eq!(d.images[2].get_pixel(2, 0).0, [0, 0, 255, 255]);
}
#[test]
fn orientation_applied_before_trim() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("oriented.jpg");
    let img = image::RgbImage::from_fn(3, 2, |x, y| {
        image::Rgb([(x * 70) as u8, (y * 100) as u8, 10])
    });
    img.save(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    let expected = image::load_from_memory(&original)
        .unwrap()
        .rotate90()
        .to_rgba8();
    let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
    let mut bytes = original[..2].to_vec();
    bytes.extend([0xff, 0xe1]);
    bytes.extend(((exif.len() + 2) as u16).to_be_bytes());
    bytes.extend(exif);
    bytes.extend(&original[2..]);
    std::fs::write(&path, bytes).unwrap();
    let d = import::decode(&path).unwrap();
    assert_eq!(d.images[0], expected);
}
use std::io::Cursor;

#[test]
fn hard_link_alias_is_rejected_even_with_force() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.png");
    RgbaImage::new(1, 1).save(&input).unwrap();
    let dest = dir.path().join("alias.pixloom");
    std::fs::hard_link(&input, &dest).unwrap();
    let error = import::run(&input, &dest, &Options::default(), true).unwrap_err();
    assert!(error.to_string().contains("aliases input"));
    assert_eq!(
        std::fs::read(&input).unwrap(),
        std::fs::read(&dest).unwrap()
    );
}
#[test]
fn animation_frame_and_total_pixel_limits_rejected_in_preflight() {
    let dir = tempfile::tempdir().unwrap();
    let gif = dir.path().join("many.gif");
    {
        let mut encoder = gif::Encoder::new(
            std::fs::File::create(&gif).unwrap(),
            1,
            1,
            &[0, 0, 0, 255, 255, 255],
        )
        .unwrap();
        for _ in 0..4097 {
            encoder
                .write_frame(&gif::Frame {
                    width: 1,
                    height: 1,
                    delay: 1,
                    buffer: std::borrow::Cow::Borrowed(&[1]),
                    ..Default::default()
                })
                .unwrap();
        }
    }
    assert!(
        import::decode(&gif)
            .err()
            .unwrap()
            .to_string()
            .contains("4096")
    );
    let path = dir.path().join("large-animation.png");
    {
        let mut encoder = png::Encoder::new(std::fs::File::create(&path).unwrap(), 1024, 1024);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_animated(65, 0).unwrap();
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&vec![0; 1024 * 1024 * 4]).unwrap();
    }
    assert!(
        import::decode(&path)
            .err()
            .unwrap()
            .to_string()
            .contains("pixel budget")
    );
}
#[test]
fn zero_duration_is_explicitly_rejected_and_atomic_staging_cleans_up() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("zero.gif");
    {
        let mut encoder = gif::Encoder::new(
            std::fs::File::create(&path).unwrap(),
            1,
            1,
            &[0, 0, 0, 255, 255, 255],
        )
        .unwrap();
        encoder
            .write_frame(&gif::Frame {
                width: 1,
                height: 1,
                buffer: std::borrow::Cow::Borrowed(&[1]),
                ..Default::default()
            })
            .unwrap();
    }
    assert!(
        import::decode(&path)
            .err()
            .unwrap()
            .to_string()
            .contains("zero-duration")
    );
    let destination = dir.path().join("preserved.pixloom");
    std::fs::write(&destination, b"preserve").unwrap();
    let staged = pixloom::output::Prepared::new(&destination, |file| {
        use std::io::Write;
        let mut writer = pixloom::output::LimitedFile::new(file, 4);
        writer.write_all(b"partial write beyond limit")?;
        Ok(())
    });
    assert!(staged.is_err());
    assert_eq!(std::fs::read(&destination).unwrap(), b"preserve");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
}
#[test]
fn apng_separate_thumbnail_is_excluded_and_animation_outputs_agree() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("with-thumbnail.png");
    {
        let mut encoder = png::Encoder::new(std::fs::File::create(&path).unwrap(), 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_animated(2, 0).unwrap();
        encoder.set_sep_def_img(true).unwrap();
        encoder.set_frame_delay(1, 10).unwrap();
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&[255, 0, 0, 255, 255, 0, 0, 255])
            .unwrap();
        writer.set_dispose_op(png::DisposeOp::Background).unwrap();
        writer
            .write_image_data(&[0, 255, 0, 255, 0, 0, 0, 0])
            .unwrap();
        writer.set_frame_dimension(1, 1).unwrap();
        writer.set_frame_position(1, 0).unwrap();
        writer.write_image_data(&[0, 0, 255, 128]).unwrap();
        writer.finish().unwrap();
    }
    let d = import::decode(&path).unwrap();
    assert_eq!(d.images.len(), 2);
    assert_eq!(d.images[0].get_pixel(0, 0).0, [0, 255, 0, 255]);
    assert_eq!(d.images[1].get_pixel(0, 0).0, [0, 0, 0, 0]);
    let expected: Vec<_> = d.images.iter().map(|i| i.as_raw().clone()).collect();
    let mut documents = vec![];
    for (n, ext) in ["pixloom.json", "pixloom"].iter().enumerate() {
        let output = dir.path().join(format!("animation.{ext}"));
        import::run(
            &path,
            &output,
            &Options {
                trim: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
        let doc = pixloom::input::read(&output).unwrap();
        assert_eq!(raw(doc.clone()), expected);
        documents.push(serde_json::to_value(doc).unwrap());
        assert!(cli(&["validate", s(&output)]).status.success());
        for command in ["build", "export"] {
            let base = dir.path().join(format!("animated{n}{command}"));
            assert!(
                cli(&[command, s(&output), "--output", s(&base)])
                    .status
                    .success()
            );
        }
    }
    assert_eq!(documents[0], documents[1]);
    let json = dir.path().join("animation.pixloom.json");
    let packed = dir.path().join("packed.pixloom");
    assert!(
        cli(&["pack", s(&json), "--output", s(&packed)])
            .status
            .success()
    );
}
