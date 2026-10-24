use pixloom::{archive::Archive, pxo, schema};
use std::{
    io::{Read, Write},
    path::Path,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

#[test]
fn duplicate_zip_names_are_rejected() {
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        for name in ["a", "b"] {
            zip.start_file(name, SimpleFileOptions::default()).unwrap();
            zip.write_all(b"x").unwrap();
        }
        zip.finish().unwrap();
    }
    let mut bytes = cursor.into_inner();
    let mut pos = 0;
    while pos + 46 < bytes.len() {
        if &bytes[pos..pos + 4] == b"PK\x01\x02" {
            if bytes[pos + 46] == b'b' {
                bytes[pos + 46] = b'a';
            }
            pos += 47;
        } else if &bytes[pos..pos + 4] == b"PK\x03\x04" {
            if bytes[pos + 30] == b'b' {
                bytes[pos + 30] = b'a';
            }
            pos += 31;
        } else {
            pos += 1;
        }
    }
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), bytes).unwrap();
    assert!(Archive::open(file.path()).is_err());
}
#[test]
fn actual_cel_data_is_required_and_indices_are_optional_for_rgba() {
    let fixture = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/pixelorama-1.2.3-static.pxo"
    ));
    for mode in ["no-indices", "short-buffer", "missing-buffer"] {
        let mut original = ZipArchive::new(std::fs::File::open(fixture).unwrap()).unwrap();
        let mut file = tempfile::NamedTempFile::new().unwrap();
        {
            let mut zip = ZipWriter::new(&mut file);
            for i in 0..original.len() {
                let mut item = original.by_index(i).unwrap();
                let name = item.name().to_owned();
                if (mode == "no-indices" && name.contains("indices_layer"))
                    || (mode == "missing-buffer" && name.ends_with("/layer_1"))
                {
                    continue;
                }
                zip.start_file(&name, SimpleFileOptions::default()).unwrap();
                if mode == "short-buffer" && name.ends_with("/layer_1") {
                    zip.write_all(&[0; 4]).unwrap();
                } else {
                    std::io::copy(&mut item, &mut zip).unwrap();
                }
            }
            zip.finish().unwrap();
        }
        assert_eq!(
            pxo::read(file.path()).is_ok(),
            mode == "no-indices",
            "{mode}"
        );
    }
}
#[test]
fn missing_cel_never_means_hold_and_invalid_numbers_fail() {
    let base = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/skills/pixloom/examples/static.json"
    ))
    .unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&base).unwrap();
    value["layers"][0]["timeline"][0]
        .as_object_mut()
        .unwrap()
        .remove("cel");
    assert!(schema::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    let invalid = String::from_utf8(base).unwrap().replace("125", "1e999");
    assert!(schema::parse(invalid.as_bytes()).is_err());
}
#[test]
fn native_palettes_and_annotations_survive_conversion() {
    let mut value: serde_json::Value = serde_json::from_slice(
        &std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/skills/pixloom/examples/static.json"
        ))
        .unwrap(),
    )
    .unwrap();
    value["pixelorama"] = serde_json::json!({"palettes":[{"Saved swatches":"{\"colors\": []}"}],"project_current_palette_name":"Saved swatches","license":"CC-BY-4.0","author_display_name":"Fixture artist"});
    value["annotations"] =
        serde_json::json!({"metadata":{"provenance":"hand-authored"},"user_data":"credit"});
    value["layers"][0]["locked"] = serde_json::json!(true);
    let s = schema::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
    let mut file = tempfile::NamedTempFile::new().unwrap();
    pxo::write(&mut file, &s).unwrap();
    let r = pxo::read(file.path()).unwrap();
    assert_eq!(r.pixelorama.palettes, s.pixelorama.palettes);
    assert_eq!(r.pixelorama.license, "CC-BY-4.0");
    assert_eq!(r.pixelorama.author_display_name, "Fixture artist");
    assert_eq!(r.annotations.user_data, "credit");
    assert_eq!(r.annotations.metadata["provenance"], "hand-authored");
    assert!(r.layers[0].locked);
    let mut zip = ZipArchive::new(std::fs::File::open(file.path()).unwrap()).unwrap();
    let mut data = String::new();
    zip.by_name("data.json")
        .unwrap()
        .read_to_string(&mut data)
        .unwrap();
    assert!(data.contains("Saved swatches"));
}

#[test]
fn native_independent_edits_split_grids_and_stale_metadata_never_creates_links() {
    let source = schema::parse(
        &std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/skills/pixloom/examples/animated.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let mut original = tempfile::NamedTempFile::new().unwrap();
    pxo::write(&mut original, &source).unwrap();
    let mut zip = ZipArchive::new(std::fs::File::open(original.path()).unwrap()).unwrap();
    let mut edited = tempfile::NamedTempFile::new().unwrap();
    {
        let mut out = ZipWriter::new(&mut edited);
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).unwrap();
            let name = entry.name().to_owned();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            if name == "data.json" {
                let mut d: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                d["layers"][0]["link_sets"][0]["cels"] = serde_json::json!([0]);
                bytes = serde_json::to_vec(&d).unwrap();
            }
            if name == "image_data/frames/3/layer_1" {
                bytes[..4].copy_from_slice(&[255, 0, 0, 255]);
            }
            out.start_file(name, SimpleFileOptions::default()).unwrap();
            out.write_all(&bytes).unwrap();
        }
        out.finish().unwrap();
    }
    let s = pxo::read(edited.path()).unwrap();
    let a = &s.layers[0].timeline[0];
    let b = &s.layers[0].timeline[2];
    assert!(a.link.is_some());
    assert!(b.link.is_none());
    assert_ne!(a.cel, b.cel);
    assert_eq!(&s.images[b.cel.as_ref().unwrap()][..4], &[255, 0, 0, 255]);
    schema::compile(schema::to_document(&s).unwrap()).unwrap();
}
