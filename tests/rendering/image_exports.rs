use image::AnimationDecoder;
use pixloom::{
    export::{self, ExportOptions, ImageFormat},
    model::Sprite,
    schema,
};
use serde_json::json;
use std::io::Cursor;

fn sprite(colors: &[&str], rows: &[Vec<u32>], durations: &[f64]) -> Sprite {
    let cels: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| json!({"id":format!("c{i}"),"pixels":[row]}))
        .collect();
    let timeline: Vec<_> = (0..durations.len())
        .map(|i| json!({"cel":format!("c{}", i % rows.len())}))
        .collect();
    schema::parse(&serde_json::to_vec(&json!({"version":1,"canvas":{"width":rows[0].len(),"height":1},
        "palette": colors, "cels":cels, "frames":durations.iter().map(|d| json!({"duration_ms":d})).collect::<Vec<_>>(),
        "layers":[{"id":"pixels","name":"Pixels","timeline":timeline}]})).unwrap()).unwrap()
}
fn encoded(s: &Sprite, format: ImageFormat, options: ExportOptions) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    export::write(&mut output, s, format, &options).unwrap();
    output.into_inner()
}

#[test]
fn lossless_still_images_preserve_rgba_and_default_to_first_frame() {
    let s = sprite(
        &["#FF0102FF", "#12345680", "#00000000", "#030405FF"],
        &[vec![0, 1, 2], vec![3, 3, 3]],
        &[125.0, 250.0],
    );
    for (format, decoder) in [
        (ImageFormat::Png, image::ImageFormat::Png),
        (ImageFormat::Tiff, image::ImageFormat::Tiff),
    ] {
        let bytes = encoded(&s, format, ExportOptions::default());
        let decoded = image::load_from_memory_with_format(&bytes, decoder)
            .unwrap()
            .into_rgba8();
        assert_eq!(decoded.dimensions(), (3, 1));
        assert_eq!(
            decoded.as_raw(),
            &[255, 1, 2, 255, 18, 52, 86, 128, 0, 0, 0, 0]
        );
        let bytes = encoded(
            &s,
            format,
            ExportOptions {
                loop_animation: false,
                frame: Some(2),
                background: None,
            },
        );
        assert_eq!(
            image::load_from_memory_with_format(&bytes, decoder)
                .unwrap()
                .into_rgba8()
                .as_raw(),
            &[3, 4, 5, 255, 3, 4, 5, 255, 3, 4, 5, 255]
        );
    }
}

#[test]
fn alpha_incompatible_formats_require_and_apply_explicit_background() {
    let s = sprite(&["#FF000080", "#00000000"], &[vec![0, 1]], &[100.0]);
    for format in [ImageFormat::Jpeg, ImageFormat::Bmp, ImageFormat::Gif] {
        assert!(export::validate(&s, format, &ExportOptions::default()).is_err());
        let mut untouched = Cursor::new(Vec::new());
        assert!(export::write(&mut untouched, &s, format, &ExportOptions::default()).is_err());
        assert!(untouched.into_inner().is_empty());
    }
    for format in [ImageFormat::Bmp, ImageFormat::Gif] {
        let bytes = encoded(
            &s,
            format,
            ExportOptions {
                loop_animation: false,
                frame: None,
                background: Some([0, 0, 255, 255]),
            },
        );
        assert_eq!(
            image::load_from_memory(&bytes)
                .unwrap()
                .into_rgba8()
                .as_raw(),
            &[128, 0, 127, 255, 0, 0, 255, 255]
        );
    }
    let solid = sprite(&["#00000000"], &[vec![0; 16]], &[100.0]);
    let bytes = encoded(
        &solid,
        ImageFormat::Jpeg,
        ExportOptions {
            loop_animation: false,
            frame: None,
            background: Some([40, 100, 180, 255]),
        },
    );
    for px in image::load_from_memory(&bytes)
        .unwrap()
        .into_rgb8()
        .pixels()
    {
        for (actual, want) in px.0.iter().zip([40i16, 100, 180]) {
            assert!((i16::from(*actual) - want).abs() <= 3);
        }
    }
}

#[test]
fn invalid_selection_and_nonopaque_background_fail_before_output() {
    let s = sprite(&["#FFFFFFFF"], &[vec![0]], &[100.0]);
    for options in [
        ExportOptions {
            loop_animation: false,
            frame: Some(0),
            background: None,
        },
        ExportOptions {
            loop_animation: false,
            frame: Some(2),
            background: None,
        },
        ExportOptions {
            loop_animation: false,
            frame: None,
            background: Some([0, 0, 0, 128]),
        },
    ] {
        let mut out = Cursor::new(Vec::new());
        assert!(export::write(&mut out, &s, ImageFormat::Png, &options).is_err());
        assert!(out.into_inner().is_empty());
    }
}

#[test]
fn svg_merges_pixel_runs_and_preserves_geometry_and_partial_opacity() {
    let s = sprite(
        &["#FF0102FF", "#12345680", "#00000000"],
        &[vec![0, 0, 2, 1]],
        &[100.0],
    );
    let text = String::from_utf8(encoded(&s, ImageFormat::Svg, ExportOptions::default())).unwrap();
    assert!(text.contains("viewBox=\"0 0 4 1\""));
    assert_eq!(text.matches("<rect ").count(), 2);
    assert!(text.contains("x=\"0\" y=\"0\" width=\"2\" height=\"1\" fill=\"#FF0102\""));
    assert!(text.contains("x=\"3\" y=\"0\" width=\"1\" height=\"1\" fill=\"#123456\""));
    let opacity = text
        .split("fill-opacity=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!((opacity - 128.0 / 255.0).abs() < 0.000001);
    assert!(!text.contains("data:image"));
}

fn animated_frames(bytes: Vec<u8>, format: ImageFormat) -> Vec<image::Frame> {
    match format {
        ImageFormat::Gif => image::codecs::gif::GifDecoder::new(Cursor::new(bytes))
            .unwrap()
            .into_frames()
            .collect_frames()
            .unwrap(),
        ImageFormat::Webp => image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))
            .unwrap()
            .into_frames()
            .collect_frames()
            .unwrap(),
        ImageFormat::Apng => image::codecs::png::PngDecoder::new(Cursor::new(bytes))
            .unwrap()
            .apng()
            .unwrap()
            .into_frames()
            .collect_frames()
            .unwrap(),
        _ => panic!("not animation"),
    }
}

#[test]
fn animations_preserve_frame_order_transparency_and_unequal_timing_once() {
    let s = sprite(
        &["#FF0000FF", "#0000FFFF", "#00000000"],
        &[vec![0, 2], vec![2, 1], vec![1, 0]],
        &[125.0, 213.0, 92.0],
    );
    for (format, want_delays) in [
        (ImageFormat::Gif, [130, 210, 90]),
        (ImageFormat::Webp, [125, 213, 92]),
        (ImageFormat::Apng, [125, 213, 92]),
    ] {
        let bytes = encoded(&s, format, ExportOptions::default());
        match format {
            ImageFormat::Gif => assert!(!bytes.windows(11).any(|w| w == b"NETSCAPE2.0")),
            ImageFormat::Webp => {
                let i = bytes.windows(4).position(|w| w == b"ANIM").unwrap();
                assert_eq!(&bytes[i + 12..i + 14], &[1, 0]);
            }
            ImageFormat::Apng => {
                let i = bytes.windows(4).position(|w| w == b"acTL").unwrap();
                assert_eq!(&bytes[i + 8..i + 12], &[0, 0, 0, 1]);
            }
            _ => unreachable!(),
        }
        let frames = animated_frames(bytes, format);
        assert_eq!(frames.len(), 3, "{format:?}");
        for (frame, want) in frames.iter().zip(want_delays) {
            let (num, den) = frame.delay().numer_denom_ms();
            assert_eq!(num as f64 / den as f64, want as f64, "{format:?}");
        }
        assert_eq!(frames[0].buffer().as_raw(), &[255, 0, 0, 255, 0, 0, 0, 0]);
        assert_eq!(frames[1].buffer().as_raw(), &[0, 0, 0, 0, 0, 0, 255, 255]);
        assert_eq!(
            frames[2].buffer().as_raw(),
            &[0, 0, 255, 255, 255, 0, 0, 255]
        );
    }
}

#[test]
fn selected_animated_format_frame_is_still_and_lossless_webp_keeps_partial_alpha() {
    let s = sprite(
        &["#FF0000FF", "#12345680"],
        &[vec![0], vec![1]],
        &[1.0, 2.0],
    );
    for format in [ImageFormat::Webp, ImageFormat::Apng] {
        let bytes = encoded(
            &s,
            format,
            ExportOptions {
                loop_animation: false,
                frame: Some(2),
                background: None,
            },
        );
        assert_eq!(
            image::load_from_memory(&bytes)
                .unwrap()
                .into_rgba8()
                .as_raw(),
            &[18, 52, 86, 128]
        );
        assert!(!bytes.windows(4).any(|w| w == b"ANIM" || w == b"acTL"));
    }
    let opaque = sprite(
        &["#FF0000FF", "#0000FFFF"],
        &[vec![0], vec![1]],
        &[1.0, 2.0],
    );
    let bytes = encoded(
        &opaque,
        ImageFormat::Gif,
        ExportOptions {
            loop_animation: false,
            frame: Some(2),
            background: None,
        },
    );
    assert_eq!(animated_frames(bytes, ImageFormat::Gif).len(), 1);
}

#[test]
fn lossless_animations_replace_partial_alpha_on_multiline_canvases() {
    let mut s = sprite(
        &["#12345680", "#FF00FF20", "#00000000"],
        &[vec![0, 1], vec![2, 0]],
        &[125.0, 250.0],
    );
    s.height = 3;
    s.images.insert(
        "c0".into(),
        [18, 52, 86, 128, 255, 0, 255, 32].repeat(3).into(),
    );
    s.images
        .insert("c1".into(), [0, 0, 0, 0, 18, 52, 86, 128].repeat(3).into());
    for format in [ImageFormat::Webp, ImageFormat::Apng] {
        let frames = animated_frames(encoded(&s, format, ExportOptions::default()), format);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].buffer().dimensions(), (2, 3));
        assert_eq!(
            frames[0].buffer().as_raw(),
            &[18, 52, 86, 128, 255, 0, 255, 32].repeat(3)
        );
        assert_eq!(
            frames[1].buffer().as_raw(),
            &[0, 0, 0, 0, 18, 52, 86, 128].repeat(3)
        );
    }
    let single = encoded(
        &s,
        ImageFormat::Webp,
        ExportOptions {
            loop_animation: false,
            frame: Some(1),
            background: None,
        },
    );
    let decoded = image::load_from_memory(&single).unwrap().into_rgba8();
    assert_eq!(decoded.dimensions(), (2, 3));
    assert_eq!(
        decoded.as_raw(),
        &[18, 52, 86, 128, 255, 0, 255, 32].repeat(3)
    );
}

#[test]
fn fractional_timing_uses_cumulative_rounding_and_rejects_zero_delay_frames() {
    let s = sprite(&["#FF0000FF"], &[vec![0]], &[1000.0 / 60.0; 60]);
    for format in [ImageFormat::Gif, ImageFormat::Webp, ImageFormat::Apng] {
        let frames = animated_frames(encoded(&s, format, ExportOptions::default()), format);
        let total: f64 = frames
            .iter()
            .map(|f| {
                let (n, d) = f.delay().numer_denom_ms();
                n as f64 / d as f64
            })
            .sum();
        assert!((total - 1000.0).abs() < 0.001, "{format:?}: {total}");
    }
    for (format, duration) in [
        (ImageFormat::Gif, 1.0),
        (ImageFormat::Webp, 0.1),
        (ImageFormat::Apng, 0.001),
    ] {
        let s = sprite(&["#FF0000FF"], &[vec![0]], &[duration, duration]);
        let mut out = Cursor::new(Vec::new());
        assert!(export::write(&mut out, &s, format, &ExportOptions::default()).is_err());
        assert!(out.into_inner().is_empty());
    }
}

#[test]
fn gif_quantizes_large_palettes_without_losing_binary_transparency() {
    let palette: Vec<_> = (0..300)
        .map(|i| format!("#{:02X}{:02X}00FF", i % 256, i / 256 * 100))
        .chain(["#00000000".into()])
        .collect();
    let s = sprite(
        &palette.iter().map(String::as_str).collect::<Vec<_>>(),
        &[(0..301).collect()],
        &[100.0],
    );
    let bytes = encoded(&s, ImageFormat::Gif, ExportOptions::default());
    let decoded = image::load_from_memory(&bytes).unwrap().into_rgba8();
    assert_eq!(decoded.dimensions(), (301, 1));
    assert!(decoded.pixels().take(300).all(|p| p[3] == 255));
    assert_eq!(decoded.get_pixel(300, 0)[3], 0);
}

#[test]
fn svg_complexity_and_unrepresentable_durations_are_preflight_errors() {
    let mut s = sprite(&["#FF0000FF"], &[vec![0]], &[100.0]);
    s.width = 512;
    s.height = 512;
    let mut rgba = Vec::new();
    for y in 0..512 {
        for x in 0..512 {
            rgba.extend_from_slice(if (x + y) % 2 == 0 {
                &[255, 0, 0, 255]
            } else {
                &[0, 0, 255, 255]
            });
        }
    }
    s.images.insert("c0".into(), rgba.into());
    assert!(export::validate(&s, ImageFormat::Svg, &ExportOptions::default()).is_err());
    for (format, duration) in [
        (ImageFormat::Gif, 655_360.0),
        (ImageFormat::Webp, 16_777_216.0),
        (ImageFormat::Apng, 65_535_001.0),
    ] {
        let s = sprite(&["#FF0000FF"], &[vec![0]], &[duration, duration]);
        assert!(
            export::validate(&s, format, &ExportOptions::default()).is_err(),
            "{format:?}"
        );
    }
}

#[test]
fn looping_exports_keep_all_frames_and_timing_and_set_format_loop_metadata() {
    let s = sprite(
        &["#FF0000FF", "#0000FFFF"],
        &[vec![0, 1], vec![1, 0]],
        &[180.0, 90.0, 210.0, 120.0],
    );
    for format in [ImageFormat::Gif, ImageFormat::Webp, ImageFormat::Apng] {
        let bytes = encoded(
            &s,
            format,
            ExportOptions {
                loop_animation: true,
                ..Default::default()
            },
        );
        match format {
            ImageFormat::Gif => {
                let mut decoder = gif::DecodeOptions::new()
                    .read_info(Cursor::new(&bytes))
                    .unwrap();
                while decoder.read_next_frame().unwrap().is_some() {}
                assert_eq!(decoder.repeat(), gif::Repeat::Infinite);
            }
            ImageFormat::Webp => {
                let i = bytes.windows(4).position(|w| w == b"ANIM").unwrap();
                assert_eq!(&bytes[i + 12..i + 14], &[0, 0]);
            }
            ImageFormat::Apng => {
                let i = bytes.windows(4).position(|w| w == b"acTL").unwrap();
                assert_eq!(&bytes[i + 8..i + 12], &[0, 0, 0, 0]);
            }
            _ => unreachable!(),
        }
        let decoded = animated_frames(bytes, format);
        assert_eq!(decoded.len(), s.frames.len());
        for (i, frame) in decoded.iter().enumerate() {
            let (n, d) = frame.delay().numer_denom_ms();
            assert_eq!(f64::from(n) / f64::from(d), s.frames[i].duration_ms);
            assert_eq!(
                frame.buffer().as_raw(),
                &pixloom::raster::render(&s, i).unwrap()
            );
        }
    }
    assert!(
        export::validate(
            &s,
            ImageFormat::Gif,
            &ExportOptions {
                loop_animation: true,
                frame: Some(1),
                ..Default::default()
            }
        )
        .is_err()
    );
}
