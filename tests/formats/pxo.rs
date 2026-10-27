use pixen::{archive::Archive, pxo, raster, schema};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    path::Path,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

fn fixture() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/pixelorama-1.2.3-static.pxo"
    ))
}
fn rewrite(path: &Path, mutate: impl FnOnce(&mut Value)) -> tempfile::NamedTempFile {
    let mut zip = ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut data = String::new();
    zip.by_name("data.json")
        .unwrap()
        .read_to_string(&mut data)
        .unwrap();
    let mut data: Value = serde_json::from_str(&data).unwrap();
    mutate(&mut data);
    let mut file = tempfile::NamedTempFile::new().unwrap();
    {
        let mut out = ZipWriter::new(&mut file);
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).unwrap();
            out.start_file(entry.name(), SimpleFileOptions::default())
                .unwrap();
            if entry.name() == "data.json" {
                out.write_all(&serde_json::to_vec(&data).unwrap()).unwrap()
            } else {
                std::io::copy(&mut entry, &mut out).unwrap();
            }
        }
        out.finish().unwrap();
    }
    file
}
#[test]
fn reads_independent_pixelorama_fixture_and_thumbnail_matches_actual_pixels() {
    let s = pxo::read(fixture()).unwrap();
    assert_eq!(
        (s.width, s.height, s.frames.len(), s.layers.len()),
        (32, 32, 1, 1)
    );
    assert_eq!(s.fps, 6.0);
    assert!((s.frames[0].duration_ms - 166.66666666666666).abs() < 1e-10);
    let id = s.layers[0].timeline[0].cel.as_ref().unwrap();
    let pixels = &s.images[id];
    assert_eq!(pixels.len(), 4096);
    assert_eq!(
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 0)
            .count(),
        532
    );
    let mut zip = ZipArchive::new(std::fs::File::open(fixture()).unwrap()).unwrap();
    let mut raw = Vec::new();
    zip.by_name("image_data/frames/1/layer_1")
        .unwrap()
        .read_to_end(&mut raw)
        .unwrap();
    assert_eq!(&**pixels, raw);
    let png = zip.by_name("preview.png").unwrap();
    let mut decoder = png::Decoder::new(png).read_info().unwrap();
    let mut buf = vec![0; decoder.output_buffer_size()];
    let info = decoder.next_frame(&mut buf).unwrap();
    assert_eq!((info.width, info.height), (256, 256));
    let actual = raster::render(&s, 0).unwrap();
    for y in 0..32 {
        for x in 0..32 {
            let a = (y * 32 + x) * 4;
            let b = (y * 8 * 256 + x * 8) * 4;
            assert_eq!(&actual[a..a + 4], &buf[b..b + 4], "{x},{y}");
        }
    }
}
#[test]
fn writer_uses_verified_paths_indices_and_duration_units() {
    let s = pxo::read(fixture()).unwrap();
    let mut f = tempfile::NamedTempFile::new().unwrap();
    pxo::write(&mut f, &s).unwrap();
    let mut z = ZipArchive::new(std::fs::File::open(f.path()).unwrap()).unwrap();
    let mut mimetype = String::new();
    z.by_name("mimetype")
        .unwrap()
        .read_to_string(&mut mimetype)
        .unwrap();
    assert_eq!(mimetype, "application/x-pixelorama");
    assert_eq!(
        z.by_name("image_data/frames/1/indices_layer_1")
            .unwrap()
            .size(),
        1
    );
    assert_eq!(
        z.by_name("image_data/frames/1/layer_1").unwrap().size(),
        4096
    );
    let mut data = String::new();
    z.by_name("data.json")
        .unwrap()
        .read_to_string(&mut data)
        .unwrap();
    let d: Value = serde_json::from_str(&data).unwrap();
    assert_eq!(d["pxo_version"], 7);
    assert_eq!(d["color_mode"], 5);
    assert_eq!(d["frames"][0]["duration"], 1.0);
    let r = pxo::read(f.path()).unwrap();
    assert_eq!(
        raster::render(&r, 0).unwrap(),
        raster::render(&s, 0).unwrap()
    );
}
#[test]
fn rejects_unsupported_features_and_inspect_reports_them() {
    for (key, value) in [
        ("blend_mode", json!(3)),
        ("type", json!(1)),
        ("parent", json!(0)),
        ("clipping_mask", json!(true)),
        ("effects", json!([{}])),
        ("animated_params", json!("{\"opacity\": {\"1\": 0.5}}")),
    ] {
        let f = rewrite(fixture(), |d| d["layers"][0][key] = value);
        assert!(pxo::read(f.path()).is_err(), "{key}");
        assert!(
            !pxo::inspect(f.path()).unwrap()["unsupported"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{key}"
        );
    }
    for (key, value) in [
        ("color_mode", json!(-1)),
        ("pxo_version", json!(6)),
        ("tilesets", json!([{}])),
        ("reference_images", json!([{}])),
    ] {
        let f = rewrite(fixture(), |d| d[key] = value);
        assert!(pxo::read(f.path()).is_err(), "{key}");
    }
    let f = rewrite(fixture(), |d| {
        d["frames"][0]["cels"][0]["z_index"] = json!(1)
    });
    assert!(pxo::read(f.path()).is_err());
}
#[test]
fn animation_links_roundtrip_keeps_independent_cels_and_empty_groups() {
    let v = json!({"version":1,"canvas":{"width":1,"height":1},"palette":["#FF000080"],"fps":8,
      "cels":[{"id":"a","pixels":[[0]]}],"frames":[{"duration_ms":125},{"duration_ms":250},{"duration_ms":375},{"duration_ms":125}],
      "layers":[{"id":"art","name":"Art","timeline":[{"cel":"a","link":"shared"},{"cel":"a"},{"cel":"a","link":"shared"},{"cel":null,"link":"empty"}]}],
      "tags":[{"name":"cycle","from":1,"to":4,"color":"#11223344"}]});
    let s = schema::parse(&serde_json::to_vec(&v).unwrap()).unwrap();
    let mut f = tempfile::NamedTempFile::new().unwrap();
    pxo::write(&mut f, &s).unwrap();
    let r = pxo::read(f.path()).unwrap();
    let t = &r.layers[0].timeline;
    assert_eq!(r.layers[0].id, "art");
    assert_eq!(t[0].link, t[2].link);
    assert!(t[1].link.is_none());
    assert!(t[3].link.is_some());
    assert_eq!(
        r.frames.iter().map(|f| f.duration_ms).collect::<Vec<_>>(),
        [125., 250., 375., 125.]
    );
    assert_eq!(r.tags[0].color, "#11223344");
    let unpacked = schema::to_document(&r).unwrap();
    let rebuilt = schema::compile(unpacked).unwrap();
    for frame in 0..4 {
        assert_eq!(
            raster::render(&rebuilt, frame).unwrap(),
            raster::render(&s, frame).unwrap()
        );
    }
}
#[test]
fn zip_safety_rejects_unsafe_paths_links_and_large_entries() {
    for name in [
        "../outside",
        "/absolute",
        "C:/drive",
        "a\\b",
        "a/../b",
        "./data.json",
    ] {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        {
            let mut z = ZipWriter::new(&mut f);
            z.start_file(name, SimpleFileOptions::default()).unwrap();
            z.write_all(b"x").unwrap();
            z.finish().unwrap();
        }
        assert!(Archive::open(f.path()).is_err(), "{name}");
    }
    let mut f = tempfile::NamedTempFile::new().unwrap();
    {
        let mut z = ZipWriter::new(&mut f);
        z.add_symlink("data.json", "outside", SimpleFileOptions::default())
            .unwrap();
        z.finish().unwrap();
    }
    assert!(Archive::open(f.path()).is_err());
}
#[test]
fn invalid_link_membership_and_timing_fail() {
    for link in [
        json!([{"cels":[0,0],"hue":0.3}]),
        json!([{"cels":[1],"hue":0.3}]),
        json!([{"cels":[0],"hue":0.3},{"cels":[0],"hue":0.6}]),
    ] {
        let f = rewrite(fixture(), |d| d["layers"][0]["link_sets"] = link);
        assert!(pxo::read(f.path()).is_err());
    }
    let f = rewrite(fixture(), |d| d["frames"][0]["duration"] = json!(0));
    assert!(pxo::read(f.path()).is_err());
}

#[test]
fn native_linking_of_null_and_explicit_zero_grids_uses_actual_pixels() {
    let source = serde_json::json!({"version":1,"canvas":{"width":1,"height":1},"palette":["#00000000"],"cels":[{"id":"blank","pixels":[[0]]}],"frames":[{"duration_ms":125},{"duration_ms":125}],"layers":[{"id":"empty","name":"Empty","timeline":[{"cel":null},{"cel":"blank"}]}]});
    let s = schema::parse(&serde_json::to_vec(&source).unwrap()).unwrap();
    let mut original = tempfile::NamedTempFile::new().unwrap();
    pxo::write(&mut original, &s).unwrap();
    let linked = rewrite(original.path(), |d| {
        d["layers"][0]["link_sets"] = serde_json::json!([{"cels":[0,1],"hue":0.3}])
    });
    let s = pxo::read(linked.path()).unwrap();
    let t = &s.layers[0].timeline;
    assert_eq!(t[0].link, t[1].link);
    assert!(t[0].link.is_some());
    assert_eq!(t[0].cel, t[1].cel);
    assert_eq!(raster::render(&s, 0).unwrap(), [0, 0, 0, 0]);
    assert_eq!(raster::render(&s, 1).unwrap(), [0, 0, 0, 0]);
    schema::compile(schema::to_document(&s).unwrap()).unwrap();
}
