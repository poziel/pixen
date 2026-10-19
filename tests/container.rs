use pixloom::{container, schema};
#[test]
fn roundtrip_and_determinism() {
    let doc = schema::read_document(include_bytes!(
        "../skills/pixloom/examples/encodings.pixloom.json"
    ))
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.pixloom");
    let mut a = std::io::Cursor::new(Vec::new());
    container::write(&mut a, &doc).unwrap();
    let mut b = std::io::Cursor::new(Vec::new());
    container::write(&mut b, &doc).unwrap();
    assert_eq!(a.get_ref(), b.get_ref());
    std::fs::write(&path, a.get_ref()).unwrap();
    let mut reader = container::Reader::open(&path).unwrap();
    assert_eq!(
        reader.load_cel(doc.cels[0].id()).unwrap().id(),
        doc.cels[0].id()
    );
    assert_eq!(
        serde_json::to_value(reader.load().unwrap()).unwrap(),
        serde_json::to_value(doc).unwrap()
    );
}

fn entries(path: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    (0..zip.len())
        .map(|i| {
            let mut e = zip.by_index(i).unwrap();
            let name = e.name().to_owned();
            let mut b = Vec::new();
            e.read_to_end(&mut b).unwrap();
            (name, b)
        })
        .collect()
}
fn archive(path: &std::path::Path, entries: &[(String, Vec<u8>)]) {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    for (name, bytes) in entries {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}
fn fixture() -> schema::Document {
    schema::read_document(include_bytes!(
        "../skills/pixloom/examples/encodings.pixloom.json"
    ))
    .unwrap()
}
#[test]
fn metadata_and_one_cel_do_not_read_unrelated_malformed_payload() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.pixloom");
    container::write(std::fs::File::create(&p).unwrap(), &fixture()).unwrap();
    let mut all = entries(&p);
    let bad = all
        .iter_mut()
        .rev()
        .find(|(n, _)| n.starts_with("cels/"))
        .unwrap();
    bad.1 = b"broken".to_vec();
    let bad_name = bad.0.clone();
    archive(&p, &all);
    let mut r = container::Reader::open(&p).unwrap();
    let good = r
        .references()
        .iter()
        .find(|r| r.payload != bad_name)
        .unwrap()
        .id
        .clone();
    r.load_cel(&good).unwrap();
    r.decode_cel(&good).unwrap();
    assert!(r.load().is_err());
}
#[test]
fn malformed_archives_and_references_fail() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.pixloom");
    container::write(std::fs::File::create(&p).unwrap(), &fixture()).unwrap();
    let original = entries(&p);
    for case in 0..10 {
        let mut all = original.clone();
        match case {
            0 => {
                all.pop();
            }
            1 => all.push(("preview.png".into(), vec![])),
            2 => all.push(("../escape".into(), vec![])),
            3 => {
                let mut m: serde_json::Value = serde_json::from_slice(&all[0].1).unwrap();
                m["container_version"] = 99.into();
                all[0].1 = serde_json::to_vec(&m).unwrap();
            }
            4 => {
                let mut m: serde_json::Value = serde_json::from_slice(&all[0].1).unwrap();
                m["required_features"] = serde_json::json!(["future"]);
                all[0].1 = serde_json::to_vec(&m).unwrap();
            }
            5..=8 => {
                let mut d: serde_json::Value = serde_json::from_slice(&all[1].1).unwrap();
                match case {
                    5 => d["cels"][1]["payload"] = d["cels"][0]["payload"].clone(),
                    6 => d["cels"][0]["payload"] = "https://external/cel.json".into(),
                    7 => d["version"] = 99.into(),
                    _ => d["canvas"]["width"] = u32::MAX.into(),
                };
                all[1].1 = serde_json::to_vec(&d).unwrap();
            }
            _ => all[0].1 = b"{".to_vec(),
        }
        archive(&p, &all);
        assert!(container::Reader::open(&p).is_err(), "case {case}");
    }
    std::fs::write(&p, b"PK\x03\x04truncated").unwrap();
    assert!(container::Reader::open(&p).is_err());
}
#[test]
fn pack_overwrite_and_input_safety_and_optimized_indices() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.json");
    let out = dir.path().join("out.pixloom");
    let bytes = serde_json::to_vec(&fixture()).unwrap();
    std::fs::write(&input, &bytes).unwrap();
    container::pack(&input, &out, true, false).unwrap();
    let packed = std::fs::read(&out).unwrap();
    assert!(container::pack(&input, &out, false, false).is_err());
    assert_eq!(packed, std::fs::read(&out).unwrap());
    let doc = container::Reader::open(&out).unwrap().load().unwrap();
    assert_eq!(
        schema::decode_indices(&fixture()).unwrap(),
        schema::decode_indices(&doc).unwrap()
    );
    assert_eq!(bytes, std::fs::read(&input).unwrap());
    std::fs::write(&input, b"bad").unwrap();
    assert!(container::pack(&input, &out, false, true).is_err());
    assert_eq!(packed, std::fs::read(&out).unwrap());
}
#[test]
fn legacy_roundtrip_preserves_unused_cels_annotations_and_links() {
    let mut d: serde_json::Value =
        serde_json::from_slice(include_bytes!("../skills/pixloom/examples/animated.json")).unwrap();
    let mut unused = d["cels"][0].clone();
    unused["id"] = "unused".into();
    d["cels"].as_array_mut().unwrap().push(unused);
    d["annotations"] = serde_json::json!({"metadata":{"test":[1,"x"]},"user_data":"hello"});
    let doc = schema::read_document(&serde_json::to_vec(&d).unwrap()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.pixloom");
    container::write(std::fs::File::create(&p).unwrap(), &doc).unwrap();
    assert_eq!(
        serde_json::to_value(&doc).unwrap(),
        serde_json::to_value(container::Reader::open(&p).unwrap().load().unwrap()).unwrap()
    );
}
#[test]
fn single_cel_decode_checks_indices_without_other_payloads() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.pixloom");
    let doc = fixture();
    container::write(std::fs::File::create(&p).unwrap(), &doc).unwrap();
    let mut r = container::Reader::open(&p).unwrap();
    let id = doc.cels[0].id();
    assert_eq!(
        r.decode_cel(id).unwrap(),
        schema::decode_indices(&doc).unwrap()[id]
    );
}
#[test]
fn cli_json_and_container_export_and_native_build_are_equivalent() {
    use std::process::Command;
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("a.pixloom.json");
    let packed = dir.path().join("a.pixloom");
    std::fs::write(
        &input,
        include_bytes!("../skills/pixloom/examples/animated.pixloom.json"),
    )
    .unwrap();
    let run = |args: Vec<String>| {
        let o = Command::new(env!("CARGO_BIN_EXE_pixloom-cli"))
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    };
    let s = |p: &std::path::Path| p.to_str().unwrap().to_owned();
    run(vec![
        "pack".into(),
        s(&input),
        "--output".into(),
        s(&packed),
    ]);
    run(vec!["validate".into(), s(&packed)]);
    run(vec!["inspect".into(), s(&packed)]);
    let unpack = dir.path().join("back.pixloom.json");
    run(vec![
        "unpack".into(),
        s(&packed),
        "--output".into(),
        s(&unpack),
    ]);
    assert_eq!(
        serde_json::to_value(pixloom::input::read(&input).unwrap()).unwrap(),
        serde_json::to_value(pixloom::input::read(&unpack).unwrap()).unwrap()
    );
    for (i, source) in [&input, &packed].iter().enumerate() {
        let base = dir.path().join(format!("export{i}"));
        run(vec![
            "export".into(),
            s(source),
            "--output".into(),
            s(&base),
            "--formats".into(),
            "png,svg,gif".into(),
        ]);
        run(vec![
            "build".into(),
            s(source),
            "--output".into(),
            s(&base),
            "--formats".into(),
            "pxo,aseprite".into(),
        ]);
    }
    for ext in ["png", "svg", "gif"] {
        assert_eq!(
            std::fs::read(dir.path().join(format!("export0.{ext}"))).unwrap(),
            std::fs::read(dir.path().join(format!("export1.{ext}"))).unwrap()
        );
    }
    for ext in ["pxo", "aseprite"] {
        let a = pixloom::native::read(&dir.path().join(format!("export0.{ext}"))).unwrap();
        let b = pixloom::native::read(&dir.path().join(format!("export1.{ext}"))).unwrap();
        assert_eq!(
            serde_json::to_value(schema::to_document(&a).unwrap()).unwrap(),
            serde_json::to_value(schema::to_document(&b).unwrap()).unwrap()
        );
    }
}
#[test]
fn zip_security_and_declared_size_limits() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.pixloom");
    container::write(std::fs::File::create(&p).unwrap(), &fixture()).unwrap();
    let original = std::fs::read(&p).unwrap();
    let central: Vec<_> = original
        .windows(4)
        .enumerate()
        .filter_map(|(i, b)| (b == b"PK\x01\x02").then_some(i))
        .collect();
    for case in 0..5 {
        let mut bytes = original.clone();
        let i = central[0];
        match case {
            0 => bytes[i + 8] |= 1, // encrypted
            1 => bytes[i + 10..i + 12].copy_from_slice(&12u16.to_le_bytes()),
            2 => bytes[i + 24..i + 28].copy_from_slice(&(65u32 * 1024 * 1024).to_le_bytes()),
            3 => {
                for &j in &central {
                    bytes[j + 24..j + 28].copy_from_slice(&(60u32 * 1024 * 1024).to_le_bytes());
                }
            }
            _ => {
                archive(&p, &[("aa".into(), vec![]), ("bb".into(), vec![])]);
                bytes = std::fs::read(&p).unwrap();
                let positions: Vec<_> = bytes
                    .windows(4)
                    .enumerate()
                    .filter_map(|(i, b)| (b == b"PK\x01\x02").then_some(i))
                    .collect();
                let second = positions[1];
                bytes[second + 46..second + 48].copy_from_slice(b"aa");
            }
        }
        std::fs::write(&p, &bytes).unwrap();
        assert!(container::Reader::open(&p).is_err(), "case {case}");
        if case == 1 {
            assert!(
                pixloom::archive::Archive::open(&p).is_err(),
                "generic native archive reader must still reject unsupported compression"
            );
        }
    }
    let file = std::fs::File::create(&p).unwrap();
    file.set_len(pixloom::archive::MAX_ARCHIVE + 1).unwrap();
    drop(file);
    assert!(container::Reader::open(&p).is_err());
}
#[test]
fn too_many_small_entries_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.pixloom");
    let all: Vec<_> = (0..schema::MAX_SLOTS + 3)
        .map(|i| (format!("cels/{i}.json"), Vec::new()))
        .collect();
    archive(&p, &all);
    assert!(format!("{:#}", container::Reader::open(&p).err().unwrap()).contains("entry count"));
}
#[test]
fn duplicate_payload_fields_are_not_silently_reinterpreted() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.pixloom");
    container::write(std::fs::File::create(&p).unwrap(), &fixture()).unwrap();
    let mut all = entries(&p);
    let e = all
        .iter_mut()
        .find(|(n, _)| n.starts_with("cels/"))
        .unwrap();
    let text = String::from_utf8(e.1.clone()).unwrap();
    e.1 = format!("{{\"encoding\":\"grid\",{}", &text[1..]).into_bytes();
    archive(&p, &all);
    assert!(container::Reader::open(&p).unwrap().load().is_err());
}
#[test]
fn corrupt_unused_payload_preserves_unpack_destination_and_source() {
    use std::process::Command;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.pixloom");
    let destination = dir.path().join("a.pixloom.json");
    container::write(std::fs::File::create(&p).unwrap(), &fixture()).unwrap();
    let mut all = entries(&p);
    all.last_mut().unwrap().1 = b"{}".to_vec();
    archive(&p, &all);
    let before = std::fs::read(&p).unwrap();
    std::fs::write(&destination, b"manual content").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_pixloom-cli"))
        .arg("unpack")
        .arg(&p)
        .arg("--output")
        .arg(&destination)
        .arg("--force")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(std::fs::read(&destination).unwrap(), b"manual content");
    assert_eq!(std::fs::read(&p).unwrap(), before);
    let out = Command::new(env!("CARGO_BIN_EXE_pixloom-cli"))
        .arg("optimize")
        .arg(&p)
        .arg("--in-place")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unpack"));
    assert_eq!(std::fs::read(&p).unwrap(), before);
}
#[test]
fn explicit_payload_references_work_independently_of_filenames_and_order() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.pixloom");
    let doc = fixture();
    container::write(std::fs::File::create(&p).unwrap(), &doc).unwrap();
    let mut all = entries(&p);
    let old = all[2].0.clone();
    all[2].0 = "cels/arbitrary.json".into();
    let mut structure: serde_json::Value = serde_json::from_slice(&all[1].1).unwrap();
    for r in structure["cels"].as_array_mut().unwrap() {
        if r["payload"] == old {
            r["payload"] = "cels/arbitrary.json".into();
        }
    }
    all[1].1 = serde_json::to_vec(&structure).unwrap();
    all.reverse();
    archive(&p, &all);
    assert_eq!(
        serde_json::to_value(doc).unwrap(),
        serde_json::to_value(container::Reader::open(&p).unwrap().load().unwrap()).unwrap()
    );
}
#[test]
fn payload_header_order_and_strict_fields_survive_direct_loading() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.pixloom");
    container::write(std::fs::File::create(&p).unwrap(), &fixture()).unwrap();
    let original = entries(&p);
    for tail in [
        r#""encoding":"grid""#,
        r#""encoding":"future""#,
        r#""encoding":null"#,
        r#""id":"injected","encoding":"grid""#,
        r#""foreign":1,"encoding":"grid""#,
    ] {
        let mut all = original.clone();
        let target = all
            .iter_mut()
            .find(|(n, _)| n.starts_with("cels/"))
            .unwrap();
        target.1 = format!("  {{\"pixels\":[[0]],{tail}}}  ").into_bytes();
        archive(&p, &all);
        let mut r = container::Reader::open(&p).unwrap();
        let id = r.references()[0].id.clone();
        if tail == r#""encoding":"grid""# {
            assert!(r.load_cel(&id).is_ok());
        } else {
            assert!(r.load_cel(&id).is_err(), "{tail}");
        }
    }
}

#[test]
fn legacy_spritex_extension_and_manifest_remain_readable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.spritex");
    let doc = fixture();
    container::write(std::fs::File::create(&path).unwrap(), &doc).unwrap();
    let mut all = entries(&path);
    let manifest = all
        .iter_mut()
        .find(|(name, _)| name == "manifest.json")
        .unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&manifest.1).unwrap();
    assert_eq!(value["format"], "pixloom");
    value["format"] = serde_json::json!("spritex");
    manifest.1 = serde_json::to_vec(&value).unwrap();
    archive(&path, &all);
    assert!(pixloom::input::is_container(&path));
    assert_eq!(
        serde_json::to_value(pixloom::input::read(&path).unwrap()).unwrap(),
        serde_json::to_value(doc).unwrap()
    );
}
