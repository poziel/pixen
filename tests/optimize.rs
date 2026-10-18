use std::process::Command;
#[test]
fn optimizer_requires_destination_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.pixloom.json");
    std::fs::write(
        &input,
        include_bytes!("../skills/pixloom/examples/encodings.pixloom.json"),
    )
    .unwrap();
    let cli = env!("CARGO_BIN_EXE_pixloom");
    assert!(
        !Command::new(cli)
            .args(["optimize"])
            .arg(&input)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new(cli)
            .args(["optimize"])
            .arg(&input)
            .arg("--in-place")
            .status()
            .unwrap()
            .success()
    );
    let first = std::fs::read(&input).unwrap();
    assert!(
        Command::new(cli)
            .args(["optimize"])
            .arg(&input)
            .arg("--in-place")
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(first, std::fs::read(input).unwrap());
}

#[test]
fn exact_indices_metadata_and_original_size_are_preserved() {
    use pixloom::{optimize, schema};
    for input in [
        include_bytes!("../skills/pixloom/examples/encodings.pixloom.json").as_slice(),
        include_bytes!("../skills/pixloom/examples/animated.pixloom.json").as_slice(),
    ] {
        let (out, report) = optimize::optimize(input).unwrap();
        optimize::verify(input, &out).unwrap();
        assert_eq!(
            schema::decode_indices(&schema::read_document(input).unwrap()).unwrap(),
            schema::decode_indices(&schema::read_document(&out).unwrap()).unwrap()
        );
        let a: serde_json::Value = serde_json::from_slice(input).unwrap();
        let b: serde_json::Value = serde_json::from_slice(&out).unwrap();
        for (x, y) in a["cels"]
            .as_array()
            .unwrap()
            .iter()
            .zip(b["cels"].as_array().unwrap())
        {
            assert!(serde_json::to_vec(y).unwrap().len() <= serde_json::to_vec(x).unwrap().len());
        }
        assert!(report.after <= report.minified_before);
        assert_eq!(out, optimize::optimize(&out).unwrap().0);
        let mut changed = b;
        changed["palette"][0] = serde_json::json!("#12345678");
        assert!(optimize::verify(input, &serde_json::to_vec(&changed).unwrap()).is_err());
    }
}

#[test]
fn collisions_invalid_input_and_verification_failures_preserve_files() {
    use pixloom::optimize;
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pixloom.json");
    let output = dir.path().join("out.pixloom.json");
    let bytes = include_bytes!("../skills/pixloom/examples/encodings.pixloom.json");
    std::fs::write(&input, bytes).unwrap();
    std::fs::write(&output, b"keep").unwrap();
    assert!(optimize::run(&input, Some(&output), false, false).is_err());
    assert!(optimize::run(&input, Some(&input), false, true).is_err());
    assert_eq!(std::fs::read(&input).unwrap(), bytes);
    assert_eq!(std::fs::read(&output).unwrap(), b"keep");
    assert!(optimize::verify(bytes, b"{}").is_err());
    std::fs::write(&input, b"{}").unwrap();
    assert!(optimize::run(&input, None, true, false).is_err());
    assert_eq!(std::fs::read(&input).unwrap(), b"{}");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn bounded_pathological_noise_and_uniform_images() {
    use pixloom::optimize;
    let mut value: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../skills/pixloom/examples/encodings.pixloom.json"
    ))
    .unwrap();
    value["canvas"] = serde_json::json!({"width":1024,"height":1024});
    value["cels"] = serde_json::json!([]);
    // Keep existing IDs and structure, but make the decoded-work budget bind.
    let original: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../skills/pixloom/examples/encodings.pixloom.json"
    ))
    .unwrap();
    let mut seed = 17u32;
    for (n, cel) in original["cels"].as_array().unwrap().iter().enumerate() {
        let pixels: Vec<Vec<u32>> = (0..1024)
            .map(|_| {
                (0..1024)
                    .map(|_| {
                        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                        if n == 0 { 0 } else { (seed >> 16) % 3 }
                    })
                    .collect()
            })
            .collect();
        value["cels"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"id":cel["id"],"encoding":"grid","pixels":pixels}));
    }
    let input = serde_json::to_vec(&value).unwrap();
    let (out, report) = optimize::optimize(&input).unwrap();
    assert!(report.skipped > 0);
    assert!(report.evaluated <= 5 * 150);
    optimize::verify(&input, &out).unwrap();
    assert_eq!(out, optimize::optimize(&out).unwrap().0);
}

#[test]
fn separate_output_extension_force_and_legacy_contract() {
    use pixloom::optimize;
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("legacy.json");
    let bytes = include_bytes!("../skills/pixloom/examples/animated.json");
    std::fs::write(&input, bytes).unwrap();
    let base = dir.path().join("optimized");
    optimize::run(&input, Some(&base), false, false).unwrap();
    let output = dir.path().join("optimized.pixloom.json");
    let first = std::fs::read(&output).unwrap();
    optimize::verify(bytes, &first).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&first).unwrap()["version"],
        1
    );
    assert!(optimize::run(&input, Some(&base), false, false).is_err());
    optimize::run(&input, Some(&output), false, true).unwrap();
    assert_eq!(first, std::fs::read(output).unwrap());
    assert_eq!(bytes, std::fs::read(input).unwrap().as_slice());
}
