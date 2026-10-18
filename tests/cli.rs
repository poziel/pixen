use std::{
    path::Path,
    process::{Command, Output},
};
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pixloom"))
        .args(args)
        .output()
        .unwrap()
}
fn input() -> &'static str {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/skills/pixloom/examples/animated.json"
    )
}
fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}
#[test]
fn cli_build_render_unpack_and_contact_sheet() {
    assert!(run(&["--version"]).status.success());
    assert!(run(&["validate", input()]).status.success());
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("a");
    let pxo = dir.path().join("a.pxo");
    let png = dir.path().join("a.png");
    let result = run(&["build", input(), "--output", s(&base)]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        run(&["export", input(), "--output", s(&base)])
            .status
            .success()
    );
    assert!(run(&["inspect", s(&pxo)]).status.success());
    let frame = dir.path().join("frame.png");
    assert!(
        run(&["render", s(&pxo), "--frame", "1", "--output", s(&frame)])
            .status
            .success()
    );
    assert_eq!(std::fs::read(&png).unwrap(), std::fs::read(&frame).unwrap());
    let second = dir.path().join("second.pxo");
    let second_png = dir.path().join("second.png");
    let second_base = dir.path().join("second");
    assert!(
        run(&["build", input(), "--output", s(&second_base)])
            .status
            .success()
    );
    assert!(
        run(&["export", input(), "--output", s(&second_base)])
            .status
            .success()
    );
    assert_eq!(std::fs::read(&pxo).unwrap(), std::fs::read(second).unwrap());
    assert_eq!(
        std::fs::read(&png).unwrap(),
        std::fs::read(second_png).unwrap()
    );
    let sheet = dir.path().join("sheet.png");
    assert!(
        run(&[
            "render",
            s(&pxo),
            "--contact-sheet",
            "--columns",
            "2",
            "--output",
            s(&sheet)
        ])
        .status
        .success()
    );
    let decoder = png::Decoder::new(std::fs::File::open(sheet).unwrap())
        .read_info()
        .unwrap();
    assert_eq!((decoder.info().width, decoder.info().height), (8, 8));
    let unpack = dir.path().join("editable.json");
    assert!(
        run(&["unpack", s(&pxo), "--output", s(&unpack)])
            .status
            .success()
    );
    assert!(run(&["validate", s(&unpack)]).status.success());
}
#[test]
fn overwrite_preflight_preserves_both_outputs_and_input() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("a");
    let pxo = dir.path().join("a.pxo");
    let png = dir.path().join("a.png");
    let svg = dir.path().join("a.svg");
    std::fs::write(&png, b"keep").unwrap();
    assert!(
        !run(&[
            "export",
            input(),
            "--output",
            s(&base),
            "--formats",
            "svg,png"
        ])
        .status
        .success()
    );
    assert!(!svg.exists());
    assert_eq!(std::fs::read(&png).unwrap(), b"keep");
    assert!(
        run(&[
            "export",
            input(),
            "--output",
            s(&base),
            "--formats",
            "svg,png",
            "--force"
        ])
        .status
        .success()
    );
    assert!(
        run(&["build", input(), "--output", s(&base)])
            .status
            .success()
    );
    let original = std::fs::read(&pxo).unwrap();
    assert!(
        !run(&["render", s(&pxo), "--output", s(&pxo), "--force"])
            .status
            .success()
    );
    assert_eq!(std::fs::read(&pxo).unwrap(), original);
    assert!(pixloom::output::preflight(Path::new(input()), &[&pxo, &pxo], true).is_err());
    assert_eq!(std::fs::read(&pxo).unwrap(), original);
}
#[test]
fn invalid_frame_and_source_do_not_leave_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("a.png");
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/pixelorama-1.2.3-static.pxo"
    );
    for f in ["0", "2"] {
        assert!(
            !run(&["render", fixture, "--frame", f, "--output", s(&output)])
                .status
                .success()
        );
        assert!(!output.exists());
    }
    let input = dir.path().join("bad.json");
    let base = dir.path().join("bad");
    std::fs::write(&input, b"{}").unwrap();
    assert!(
        !run(&["build", s(&input), "--output", s(&base)])
            .status
            .success()
    );
    assert!(!output.exists());
    assert!(!base.with_extension("pxo").exists());
}

#[test]
#[cfg(windows)]
fn output_aliases_with_different_case_are_rejected_before_creation() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("alias.pxo");
    let b = dir.path().join("ALIAS.PXO");
    assert!(pixloom::output::preflight(Path::new(input()), &[&a, &b], true).is_err());
    assert!(!a.exists());
}

#[test]
#[cfg(windows)]
fn ambiguous_windows_output_names_cannot_replace_the_other_output() {
    for name in ["alias.pxo.", "alias.pxo ", "alias.pxo:preview"] {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("alias.pxo");
        let b = dir.path().join(name);
        assert!(
            pixloom::output::preflight(Path::new(input()), &[&a, &b], true).is_err(),
            "{name}"
        );
        assert!(!a.exists());
    }
}
