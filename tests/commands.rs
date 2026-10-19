use std::{
    path::Path,
    process::{Command, Output},
};
fn input() -> &'static str {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/skills/pixloom/examples/animated.json"
    )
}
fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pixloom-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn ok(args: &[&str]) {
    let out = run(args);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn renamed_commands_build_both_projects_export_all_images_and_unpack_json_files() {
    assert!(String::from_utf8_lossy(&run(&["--version"]).stdout).starts_with("pixloom-cli 0.2"));
    assert!(String::from_utf8_lossy(&run(&["--help"]).stdout).contains("export"));
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("art");
    ok(&[
        "build",
        input(),
        "--output",
        s(&base),
        "--formats",
        "pxo,aseprite",
    ]);
    for ext in ["pxo", "aseprite"] {
        let file = base.with_extension(ext);
        ok(&["inspect", s(&file)]);
        let json = dir.path().join(format!("{ext}.json"));
        ok(&["unpack", s(&file), "--output", s(&json)]);
        assert!(json.is_file());
        ok(&["validate", s(&json)]);
        let sheet = dir.path().join(format!("{ext}.png"));
        ok(&[
            "render",
            s(&file),
            "--contact-sheet",
            "--columns",
            "2",
            "--output",
            s(&sheet),
        ]);
        let decoded = image::open(sheet).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (8, 8));
    }
    let base = dir.path().join("images");
    ok(&[
        "export",
        input(),
        "--output",
        s(&base),
        "--formats",
        "PNG,svg,jpg,JPEG,bmp,tif,TIFF,gif,webp,apng",
        "--background",
        "#FFFFFF",
    ]);
    for ext in ["png", "svg", "jpeg", "bmp", "tiff", "gif", "webp", "apng"] {
        assert!(base.with_extension(ext).is_file(), "{ext}");
    }
    assert!(!base.with_extension("jpg").exists());
    assert!(!base.with_extension("tif").exists());
}
#[test]
fn all_outputs_are_preflighted_and_encoder_failures_install_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("art");
    let svg = base.with_extension("svg");
    std::fs::write(&svg, b"keep").unwrap();
    assert!(
        !run(&[
            "export",
            input(),
            "--output",
            s(&base),
            "--formats",
            "png,svg"
        ])
        .status
        .success()
    );
    assert!(!base.with_extension("png").exists());
    assert_eq!(std::fs::read(&svg).unwrap(), b"keep");
    ok(&[
        "export",
        input(),
        "--output",
        s(&base),
        "--formats",
        "png,svg",
        "--force",
    ]);
    let missing = dir.path().join("no-ffmpeg.exe");
    let base = dir.path().join("failed");
    assert!(
        !run(&[
            "export",
            input(),
            "--output",
            s(&base),
            "--formats",
            "png,mp4",
            "--ffmpeg",
            s(&missing),
            "--background",
            "#FFFFFFFF"
        ])
        .status
        .success()
    );
    assert!(!base.with_extension("png").exists());
    assert!(!base.with_extension("mp4").exists());
    ok(&[
        "export",
        input(),
        "--output",
        s(&base),
        "--formats",
        "png",
        "--ffmpeg",
        s(&missing),
    ]);
    let source = dir.path().join("fractional.json");
    let mut v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(input()).unwrap()).unwrap();
    v["frames"][0]["duration_ms"] = serde_json::json!(125.5);
    std::fs::write(&source, serde_json::to_vec(&v).unwrap()).unwrap();
    assert!(
        !run(&[
            "build",
            s(&source),
            "--output",
            s(&base),
            "--formats",
            "pxo,aseprite"
        ])
        .status
        .success()
    );
    assert!(!base.with_extension("pxo").exists());
    assert!(!base.with_extension("aseprite").exists());
    assert!(
        std::fs::read_dir(dir.path()).unwrap().all(|f| !f
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".tmp"))
    );
}
#[test]
fn explicit_frames_format_categories_and_native_signatures_are_checked() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("frame");
    ok(&[
        "export",
        input(),
        "--output",
        s(&base),
        "--formats",
        "png,gif,webp,apng",
        "--frame",
        "2",
    ]);
    let p = image::open(base.with_extension("png"))
        .unwrap()
        .into_rgba8();
    assert_eq!(p.get_pixel(1, 1).0, [250, 240, 202, 255]);
    let other = dir.path().join("invalid");
    for (formats, frame) in [("png,mp4", "2"), ("png", "0"), ("nonesuch", "1")] {
        assert!(
            !run(&[
                "export",
                input(),
                "--output",
                s(&other),
                "--formats",
                formats,
                "--frame",
                frame
            ])
            .status
            .success()
        );
        assert!(!other.with_extension("png").exists());
    }
    assert!(
        !run(&["build", input(), "--output", s(&other), "--formats", "png"])
            .status
            .success()
    );
    assert!(
        !run(&[
            "export",
            input(),
            "--output",
            s(&other.with_extension("png")),
            "--formats",
            "png"
        ])
        .status
        .success()
    );
    ok(&[
        "build",
        input(),
        "--output",
        s(&other),
        "--formats",
        "aseprite",
    ]);
    let alias = other.with_extension("ase");
    std::fs::copy(other.with_extension("aseprite"), &alias).unwrap();
    ok(&["inspect", s(&alias)]);
    let mismatch = other.with_extension("pxo");
    std::fs::copy(&alias, &mismatch).unwrap();
    let result = run(&["inspect", s(&mismatch)]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("mismatch"));
}

#[test]
#[ignore = "requires FFmpeg with libx264rgb/libvpx-vp9 and ffprobe"]
fn mixed_static_animation_and_video_cli_exports_one_complete_timeline() {
    use image::AnimationDecoder;
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("mixed");
    ok(&[
        "export",
        input(),
        "--output",
        s(&base),
        "--formats",
        "png,gif,webp,apng,mp4,webm",
        "--background",
        "#203040FF",
    ]);
    let sprite = pixloom::schema::parse(&std::fs::read(input()).unwrap()).unwrap();
    let expected = pixloom::raster::opaque_rgb(
        &pixloom::raster::render(&sprite, 0).unwrap(),
        Some([32, 48, 64, 255]),
    )
    .unwrap();
    assert_eq!(
        image::open(base.with_extension("png"))
            .unwrap()
            .into_rgb8()
            .into_raw(),
        expected
    );
    let gif = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(
        std::fs::File::open(base.with_extension("gif")).unwrap(),
    ))
    .unwrap();
    assert_eq!(
        gif.into_frames().collect_frames().unwrap().len(),
        sprite.frames.len()
    );
    for ext in ["mp4", "webm"] {
        let output = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-count_frames",
                "-show_entries",
                "stream=width,height,nb_read_frames:format=duration",
                "-of",
                "json",
            ])
            .arg(base.with_extension(ext))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["streams"][0]["width"], 4);
        assert_eq!(report["streams"][0]["height"], 4);
        assert_eq!(report["streams"][0]["nb_read_frames"], "4");
        let seconds: f64 = report["format"]["duration"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!((seconds - 0.625).abs() < 0.0001, "{ext}: {seconds}");
    }
    assert!(
        std::fs::read_dir(dir.path()).unwrap().all(|f| !f
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".tmp"))
    );
}
