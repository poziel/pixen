use pixloom::{
    model::Sprite,
    raster, schema,
    video::{self, VideoFormat, VideoOptions},
};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

fn animation(durations: &[f64], alpha: bool) -> Sprite {
    let palette = if alpha {
        vec!["#FF115DFF", "#13F57A80", "#4939ED00"]
    } else {
        vec!["#FF115DFF", "#13F57AFF", "#4939EDFF"]
    };
    let cels: Vec<_> = (0..durations.len()).map(|frame| json!({
        "id": format!("cel-{frame}"),
        "pixels": (0..3).map(|y| (0..3).map(|x| (x + y + frame) % 3).collect::<Vec<_>>()).collect::<Vec<_>>()
    })).collect();
    let document = json!({
        "version": 1, "canvas": {"width": 3, "height": 3}, "palette": palette,
        "cels": cels,
        "frames": durations.iter().map(|ms| json!({"duration_ms":ms})).collect::<Vec<_>>(),
        "layers": [{"id":"art", "name":"Art", "timeline": (0..durations.len()).map(|frame| json!({"cel":format!("cel-{frame}")})).collect::<Vec<_>>() }]
    });
    schema::parse(&serde_json::to_vec(&document).unwrap()).unwrap()
}

fn missing_options(parent: &Path) -> VideoOptions {
    VideoOptions {
        ffmpeg: parent.join("not-installed-ffmpeg.exe"),
        temp_parent: Some(parent.to_owned()),
        ..Default::default()
    }
}

#[test]
fn missing_ffmpeg_is_actionable_and_leaves_output_and_temp_parent_untouched() {
    let parent = tempfile::tempdir().unwrap();
    let mut destination = tempfile::NamedTempFile::new().unwrap();
    destination.write_all(b"untouched").unwrap();
    let options = missing_options(parent.path());
    let error = video::write(
        destination.as_file_mut(),
        &animation(&[13.0, 127.0], false),
        VideoFormat::Mp4,
        &options,
    )
    .unwrap_err();
    let error = format!("{error:#}");
    assert!(
        error.contains("install")
            && error.contains("--ffmpeg PATH")
            && error.contains("libx264rgb"),
        "{error}"
    );
    assert_eq!(fs::read(destination.path()).unwrap(), b"untouched");
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
}

#[test]
fn invalid_source_timing_background_and_storage_fail_before_dependency_lookup() {
    let parent = tempfile::tempdir().unwrap();
    let mut options = missing_options(parent.path());
    for (source, expected) in [
        (animation(&[125.0], false), "at least two frames"),
        (animation(&[0.4, 1.0], false), "1 ms precision"),
        (animation(&[1.0, 86_400_000.0], false), "24 hour"),
        (animation(&[13.0, 127.0], true), "opaque background"),
    ] {
        let error = video::validate(&source, VideoFormat::Webm, &options)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
    }
    options.background = Some([0, 0, 0, 128]);
    assert!(
        video::validate(&animation(&[1.0, 1.0], false), VideoFormat::Mp4, &options)
            .unwrap_err()
            .to_string()
            .contains("must be opaque")
    );
    options.background = None;
    let mut oversized = animation(&[1.0; 32], false);
    oversized.width = 4096;
    oversized.height = 1024;
    let image: std::sync::Arc<[u8]> = vec![255; 4096 * 1024 * 4].into();
    for value in oversized.images.values_mut() {
        *value = image.clone();
    }
    assert!(
        video::validate(&oversized, VideoFormat::Mp4, &options)
            .unwrap_err()
            .to_string()
            .contains("256 MiB")
    );
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
}

#[test]
fn transparency_is_checked_in_later_frames_and_after_layer_compositing() {
    let parent = tempfile::tempdir().unwrap();
    let options = missing_options(parent.path());
    let mut source = animation(&[13.0, 127.0], false);
    source.layers[0].timeline[1].opacity = 0.5;
    let error = video::validate(&source, VideoFormat::Mp4, &options)
        .unwrap_err()
        .to_string();
    assert!(error.contains("frame 2 has transparency"), "{error}");
    let mut background_layer = source.layers[0].clone();
    background_layer.id = "back".into();
    background_layer.timeline[1].opacity = 1.0;
    source.layers.insert(0, background_layer);
    // An opaque rendered composition reaches the dependency lookup without a background.
    let error = video::validate(&source, VideoFormat::Mp4, &options)
        .unwrap_err()
        .to_string();
    assert!(error.contains("FFmpeg"), "{error}");
}

fn fake_ffmpeg(directory: &Path) -> PathBuf {
    let source = directory.join("helper.rs");
    fs::write(&source, r#"
        fn main() {
            let args: Vec<_> = std::env::args().collect();
            let exe = std::env::current_exe().unwrap();
            let name = exe.file_name().unwrap().to_string_lossy();
            if args.iter().any(|a| a == "-encoders") {
                if name.contains("missing") { println!(" V..... unrelated_encoder"); }
                else { println!(" V..... libx264rgb\n V..... libvpx-vp9"); }
                return;
            }
            std::fs::write(exe.parent().unwrap().join("arguments.txt"), format!("{args:?}")).unwrap();
            let file = std::fs::File::create("encoded.mp4").unwrap();
            if name.contains("limit") { file.set_len(128 * 1024 * 1024).unwrap(); return; }
            eprintln!("simulated encoder failure");
            std::process::exit(17);
        }
    "#).unwrap();
    let executable = directory.join(if cfg!(windows) {
        "ffmpeg ' & $ failure.exe"
    } else {
        "ffmpeg ' & $ failure"
    });
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc)
        .arg("--crate-name")
        .arg("pixloom_video_test_helper")
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    executable
}

#[test]
fn encoder_failures_and_output_limit_clean_temporary_frames_without_touching_destination() {
    let directory = tempfile::tempdir().unwrap();
    let parent = directory.path().join("temporary ' & $ files");
    fs::create_dir(&parent).unwrap();
    let executable = fake_ffmpeg(directory.path());
    let mut options = VideoOptions {
        ffmpeg: executable.clone(),
        temp_parent: Some(parent.clone()),
        ..Default::default()
    };
    let mut destination = tempfile::NamedTempFile::new().unwrap();
    destination.write_all(b"untouched").unwrap();
    let source = animation(&[13.0, 127.0, 259.0], false);
    let error = video::write(
        destination.as_file_mut(),
        &source,
        VideoFormat::Mp4,
        &options,
    )
    .unwrap_err()
    .to_string();
    assert!(
        error.contains("libx264rgb") && error.contains("simulated encoder failure"),
        "{error}"
    );
    assert_eq!(fs::read(destination.path()).unwrap(), b"untouched");
    assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);
    let arguments = fs::read_to_string(directory.path().join("arguments.txt")).unwrap();
    assert!(
        arguments.contains("\"-nostdin\"")
            && arguments.contains("\"frames.ffconcat\"")
            && arguments.contains("\"encoded.mp4\""),
        "{arguments}"
    );
    assert!(!arguments.contains("cmd.exe") && !arguments.contains("/bin/sh"));

    let limited = directory
        .path()
        .join(if cfg!(windows) { "limit.exe" } else { "limit" });
    fs::copy(&executable, &limited).unwrap();
    options.ffmpeg = limited;
    let error = format!(
        "{:#}",
        video::write(
            destination.as_file_mut(),
            &source,
            VideoFormat::Mp4,
            &options
        )
        .unwrap_err()
    );
    assert!(error.contains("limit"), "{error}");
    assert_eq!(fs::read(destination.path()).unwrap(), b"untouched");
    assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);

    let missing = directory.path().join(if cfg!(windows) {
        "missing.exe"
    } else {
        "missing"
    });
    fs::copy(&executable, &missing).unwrap();
    options.ffmpeg = missing;
    let error = video::validate(&source, VideoFormat::Webm, &options)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("libvpx-vp9") && error.contains("install"),
        "{error}"
    );
    assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);
}

fn probe(path: &Path) -> Value {
    let result = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "frame=pts_time,duration_time:format=duration:stream=width,height,pix_fmt,codec_name",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}

fn numeric(value: &Value) -> f64 {
    value.as_str().unwrap().parse().unwrap()
}

fn assert_timeline(info: &Value, source: &Sprite) {
    let frames = info["frames"].as_array().unwrap();
    assert_eq!(frames.len(), source.frames.len(), "{info}");
    let mut exact_elapsed: f64 = 0.0;
    let mut prior = 0.0;
    for (encoded, original) in frames.iter().zip(&source.frames) {
        exact_elapsed += original.duration_ms;
        let endpoint = exact_elapsed.round();
        assert!(
            (numeric(&encoded["pts_time"]) * 1000.0 - prior).abs() < 0.001,
            "{info}"
        );
        assert!(
            (numeric(&encoded["duration_time"]) * 1000.0 - (endpoint - prior)).abs() < 0.001,
            "{info}"
        );
        prior = endpoint;
    }
    assert!(
        (numeric(&info["format"]["duration"]) * 1000.0 - exact_elapsed).abs() <= 0.500_001,
        "{info}"
    );
}

fn decode(path: &Path) -> Vec<u8> {
    let result = Command::new("ffmpeg")
        .args(["-v", "error", "-nostdin", "-i"])
        .arg(path)
        .args([
            "-fps_mode",
            "passthrough",
            "-pix_fmt",
            "rgba",
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}

#[test]
#[ignore = "requires FFmpeg with libx264rgb/libvpx-vp9 and ffprobe; run --ignored"]
fn installed_ffmpeg_preserves_odd_dimensions_full_timeline_and_unequal_durations() {
    let directory = tempfile::tempdir().unwrap();
    let parent = directory.path().join("temporary ' & $ files");
    fs::create_dir(&parent).unwrap();
    let source = animation(&[13.0, 127.0, 259.0], false);
    let options = VideoOptions {
        temp_parent: Some(parent.clone()),
        ..Default::default()
    };
    for (format, extension, codec, pixels) in [
        (VideoFormat::Mp4, "mp4", "h264", "gbrp"),
        (VideoFormat::Webm, "webm", "vp9", "yuv444p"),
    ] {
        let path = directory.path().join(format!("result.{extension}"));
        video::write(
            &mut fs::File::create(&path).unwrap(),
            &source,
            format,
            &options,
        )
        .unwrap();
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);
        let info = probe(&path);
        assert_timeline(&info, &source);
        assert_eq!(info["streams"][0]["width"], 3);
        assert_eq!(info["streams"][0]["height"], 3);
        assert_eq!(info["streams"][0]["codec_name"], codec);
        assert_eq!(info["streams"][0]["pix_fmt"], pixels);
        let decoded = decode(&path);
        let expected: Vec<_> = (0..source.frames.len())
            .flat_map(|frame| raster::render(&source, frame).unwrap())
            .collect();
        assert_eq!(decoded.len(), expected.len());
        if format == VideoFormat::Mp4 {
            assert_eq!(decoded, expected);
        } else {
            // VP9 is lossless in YUV; the RGB/YUV conversion can still round channels.
            assert!(
                decoded
                    .iter()
                    .zip(expected)
                    .all(|(a, b)| a.abs_diff(b) <= 3)
            );
            assert!(
                decoded
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|pixel| pixel[3] == 255)
            );
        }
        println!(
            "{extension}: frames=3 canvas=3x3 timestamps=0,13,140ms durations=13,127,259ms total=399ms {codec}/{pixels}"
        );
    }
}

#[test]
#[ignore = "requires FFmpeg with libx264rgb/libvpx-vp9 and ffprobe; run --ignored"]
fn installed_ffmpeg_rounds_cumulative_timing_and_composites_explicit_background_exactly() {
    let directory = tempfile::tempdir().unwrap();
    let source = animation(&[1000.0 / 60.0; 30], true);
    let options = VideoOptions {
        background: Some([10, 20, 30, 255]),
        ..Default::default()
    };
    for (format, extension) in [(VideoFormat::Mp4, "mp4"), (VideoFormat::Webm, "webm")] {
        let path = directory.path().join(format!("rounded.{extension}"));
        video::write(
            &mut fs::File::create(&path).unwrap(),
            &source,
            format,
            &options,
        )
        .unwrap();
        assert_timeline(&probe(&path), &source);
        let decoded = decode(&path);
        assert_eq!(decoded.len(), 30 * 3 * 3 * 4);
        assert!(
            decoded
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[3] == 255)
        );
        if format == VideoFormat::Mp4 {
            let expected: Vec<_> = (0..source.frames.len())
                .flat_map(|frame| {
                    raster::opaque_rgb(&raster::render(&source, frame).unwrap(), options.background)
                        .unwrap()
                        .as_chunks::<3>()
                        .0
                        .iter()
                        .flat_map(|p| [p[0], p[1], p[2], 255])
                        .collect::<Vec<_>>()
                })
                .collect();
            assert_eq!(decoded, expected);
        }
        println!(
            "{extension}: 30 fractional 60fps durations retain 30 frames and total500ms; explicit background opaque"
        );
    }
}
