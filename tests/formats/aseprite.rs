use pixen::{aseprite, pxo, raster, schema};
use serde_json::json;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/aseprite")
        .join(name)
}
fn model() -> pixen::model::Sprite {
    schema::parse(&serde_json::to_vec(&json!({"version":1,"canvas":{"width":2,"height":1},"palette":["#FA123480","#12345600"],"fps":8,
        "cels":[{"id":"paint","pixels":[[0,1]]}],
        "annotations":{"metadata":{"source":{"tool":"test"}},"user_data":"project note"},
        "pixelorama":{"author_display_name":"Artist"},
        "frames":[{"duration_ms":125,"annotations":{"user_data":"frame note"}},{"duration_ms":250},{"duration_ms":375},{"duration_ms":125},{"duration_ms":125},{"duration_ms":125}],
        "layers":[{"id":"art","name":"Art","visible":true,"locked":true,"new_cels_linked":true,"annotations":{"metadata":{"kind":"raster"},"user_data":"layer note"},"timeline":[
            {"cel":"paint","annotations":{"metadata":{"test":true},"user_data":"cel note"}},
            {"cel":"paint"},{"cel":"paint","link":"held"},{"cel":"paint","link":"held"},
            {"cel":"paint","link":"singleton"},{"cel":null,"link":"empty","opacity":0.5,"annotations":{"user_data":"blank note"}}]}],
        "tags":[{"name":"cycle","from":1,"to":6,"color":"#11223344","user_data":"tag note"}]
    })).unwrap()).unwrap()
}
fn written(s: &pixen::model::Sprite) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    aseprite::write(&mut f, s).unwrap();
    f
}
fn temp(bytes: &[u8]) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(bytes).unwrap();
    f
}
fn chunks(bytes: &[u8]) -> Vec<(usize, usize, u16, usize)> {
    let mut out = Vec::new();
    let mut frame = 128;
    while frame < bytes.len() {
        let size = u32::from_le_bytes(bytes[frame..frame + 4].try_into().unwrap()) as usize;
        let mut c = frame + 16;
        while c < frame + size {
            let n = u32::from_le_bytes(bytes[c..c + 4].try_into().unwrap()) as usize;
            let kind = u16::from_le_bytes(bytes[c + 4..c + 6].try_into().unwrap());
            out.push((c, n, kind, frame));
            c += n;
        }
        frame += size;
    }
    out
}

fn replace_chunk(base: &[u8], old: &(usize, usize, u16, usize), payload: &[u8]) -> Vec<u8> {
    let mut chunk = Vec::new();
    chunk.extend_from_slice(&((payload.len() + 6) as u32).to_le_bytes());
    chunk.extend_from_slice(&old.2.to_le_bytes());
    chunk.extend_from_slice(payload);
    let mut bytes = base.to_vec();
    bytes.splice(old.0..old.0 + old.1, chunk);
    let frame_size = u32::from_le_bytes(base[old.3..old.3 + 4].try_into().unwrap()) as usize;
    let total = bytes.len() as u32;
    bytes[..4].copy_from_slice(&total.to_le_bytes());
    bytes[old.3..old.3 + 4]
        .copy_from_slice(&((frame_size - old.1 + payload.len() + 6) as u32).to_le_bytes());
    bytes
}

#[test]
fn independently_saved_cropped_rgba_matches_upstream_png() {
    let s = aseprite::read(&fixture("basic-16x16.aseprite")).unwrap();
    assert_eq!(
        (s.width, s.height, s.frames.len(), s.layers.len()),
        (16, 16, 1, 1)
    );
    assert_eq!(s.layers[0].name, "Layer 1");
    assert_eq!(s.frames[0].duration_ms, 100.0);
    let png = image::open(fixture("basic-16x16.png")).unwrap().to_rgba8();
    assert_eq!(raster::render(&s, 0).unwrap(), png.into_raw());
    let round = aseprite::read(written(&s).path()).unwrap();
    assert_eq!(
        raster::render(&round, 0).unwrap(),
        raster::render(&s, 0).unwrap()
    );
    assert_eq!(
        s.annotations.metadata["aseprite_native"]["chunks"],
        round.annotations.metadata["aseprite_native"]["chunks"]
    );
}

#[test]
fn independently_saved_links_match_upstream_renders_and_editing_groups() {
    let s = aseprite::read(&fixture("linked_cels.aseprite")).unwrap();
    assert_eq!(
        (s.width, s.height, s.frames.len(), s.layers.len()),
        (16, 16, 3, 3)
    );
    let a = &s.layers[0].timeline;
    assert!(a[0].link.is_some());
    assert_eq!(a[0].link, a[1].link);
    assert_eq!(a[0].link, a[2].link);
    let b = &s.layers[1].timeline;
    assert_eq!(b[0].link, b[2].link);
    assert!(b[0].link.is_some());
    assert!(b[1].link.is_none());
    for frame in 0..3 {
        let png = image::open(fixture(&format!("linked_cels_{:02}.png", frame + 1)))
            .unwrap()
            .to_rgba8();
        assert_eq!(raster::render(&s, frame).unwrap(), png.into_raw());
    }
}

#[test]
fn authoring_roundtrip_preserves_rgba_metadata_timing_tags_and_explicit_links() {
    let s = model();
    let f = written(&s);
    let r = aseprite::read(f.path()).unwrap();
    assert_eq!(r.fps, s.fps);
    assert_eq!(r.layers[0].id, "art");
    assert!(r.layers[0].locked);
    assert!(r.layers[0].new_cels_linked);
    assert_eq!(r.annotations.user_data, s.annotations.user_data);
    assert_eq!(
        r.annotations.metadata["source"],
        s.annotations.metadata["source"]
    );
    assert_eq!(
        serde_json::to_value(&r.pixelorama).unwrap(),
        serde_json::to_value(&s.pixelorama).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&r.frames).unwrap(),
        serde_json::to_value(&s.frames).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&r.tags).unwrap(),
        serde_json::to_value(&s.tags).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&r.layers).unwrap(),
        serde_json::to_value(&s.layers).unwrap()
    );
    let t = &r.layers[0].timeline;
    assert!(t[0].link.is_none() && t[1].link.is_none());
    assert_eq!(t[2].link.as_deref(), Some("held"));
    assert_eq!(t[2].link, t[3].link);
    assert_eq!(t[4].link.as_deref(), Some("singleton"));
    assert_eq!(t[5].link.as_deref(), Some("empty"));
    assert_eq!(&*r.images["paint"], &*s.images["paint"]);
    let doc = schema::to_document(&r).unwrap();
    let reparsed = schema::compile(doc).unwrap();
    for frame in 0..s.frames.len() {
        assert_eq!(
            raster::render(&s, frame).unwrap(),
            raster::render(&reparsed, frame).unwrap()
        );
    }
}

#[test]
fn raw_headers_and_chunks_use_native_links_only_for_declared_groups() {
    let s = model();
    let bytes = std::fs::read(written(&s).path()).unwrap();
    assert_eq!(
        u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize,
        bytes.len()
    );
    assert_eq!(&bytes[4..6], &[0xe0, 0xa5]);
    assert_eq!(&bytes[12..14], &32u16.to_le_bytes());
    let types: Vec<_> = chunks(&bytes)
        .iter()
        .filter(|c| c.2 == 0x2005)
        .map(|(p, _, _, _)| u16::from_le_bytes(bytes[p + 13..p + 15].try_into().unwrap()))
        .collect();
    assert_eq!(types, [2, 2, 2, 1, 2, 2]);
    let layer = chunks(&bytes)
        .into_iter()
        .find(|c| c.2 == 0x2004)
        .unwrap()
        .0;
    assert_eq!(
        u16::from_le_bytes(bytes[layer + 6..layer + 8].try_into().unwrap()),
        17
    );
    let durations: Vec<_> = {
        let mut out = Vec::new();
        let mut p = 128;
        while p < bytes.len() {
            out.push(u16::from_le_bytes(bytes[p + 8..p + 10].try_into().unwrap()));
            p += u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap()) as usize;
        }
        out
    };
    assert_eq!(durations, [125, 250, 375, 125, 125, 125]);
}

#[test]
fn opacity_rounds_to_nearest_native_byte_and_fractional_timing_is_rejected() {
    let mut s = model();
    s.layers[0].opacity = 0.5;
    s.layers[0].timeline[0].opacity = 0.1;
    let r = aseprite::read(written(&s).path()).unwrap();
    assert_eq!(r.layers[0].opacity, 128.0 / 255.0);
    assert_eq!(r.layers[0].timeline[0].opacity, 26.0 / 255.0);
    for duration in [0.25, 125.5, 65536.0] {
        s.frames[0].duration_ms = duration;
        assert!(aseprite::write(std::io::Cursor::new(Vec::new()), &s).is_err());
    }
    s.frames[0].duration_ms = 65535.0;
    assert!(aseprite::write(std::io::Cursor::new(Vec::new()), &s).is_ok());
}

#[test]
fn native_to_pxo_to_native_preserves_supported_model_without_png_intermediate() {
    let s = model();
    let mut p = tempfile::NamedTempFile::new().unwrap();
    pxo::write(&mut p, &s).unwrap();
    let from_pxo = pxo::read(p.path()).unwrap();
    let from_ase = aseprite::read(written(&from_pxo).path()).unwrap();
    for f in 0..s.frames.len() {
        assert_eq!(
            raster::render(&from_ase, f).unwrap(),
            raster::render(&s, f).unwrap()
        );
    }
    assert_eq!(
        from_ase
            .frames
            .iter()
            .map(|f| f.duration_ms)
            .collect::<Vec<_>>(),
        s.frames.iter().map(|f| f.duration_ms).collect::<Vec<_>>()
    );
    assert_eq!(
        from_ase.layers[0].timeline[2].link,
        from_ase.layers[0].timeline[3].link
    );
    assert!(from_ase.layers[0].timeline[0].link.is_none());
    assert_eq!(from_ase.pixelorama.author_display_name, "Artist");
}

#[test]
fn unsupported_independent_files_fail_read_and_are_reported_by_inspect() {
    for name in [
        "grayscale.aseprite",
        "rawcel.aseprite",
        "layers_and_tags.aseprite",
        "slice.aseprite",
        "tilemap.aseprite",
        "user_data.aseprite",
    ] {
        assert!(aseprite::read(&fixture(name)).is_err(), "{name}");
        let info = aseprite::inspect(&fixture(name)).unwrap();
        assert_eq!(info["raster_validated"], false);
        assert!(
            !info["unsupported_features"].as_array().unwrap().is_empty(),
            "{name}"
        );
    }
}

#[test]
fn malformed_native_headers_chunks_links_and_zlib_are_bounded() {
    let base = std::fs::read(written(&model()).path()).unwrap();
    for len in [0, 127, 128, base.len() - 1] {
        assert!(aseprite::read(temp(&base[..len]).path()).is_err());
    }
    for (offset, value) in [(4, 0u16), (6, 4097u16), (8, 4097u16), (128 + 8, 0u16)] {
        let mut b = base.clone();
        b[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        assert!(aseprite::read(temp(&b).path()).is_err());
    }
    let all = chunks(&base);
    let linked = all
        .iter()
        .find(|(p, _, ty, _)| *ty == 0x2005 && base[*p + 13] == 1)
        .unwrap();
    for target in [3u16, 65535u16] {
        let mut b = base.clone();
        b[linked.0 + 22..linked.0 + 24].copy_from_slice(&target.to_le_bytes());
        assert!(aseprite::read(temp(&b).path()).is_err());
    }
    let cel = all.iter().find(|c| c.2 == 0x2005).unwrap();
    for offset in [cel.0, cel.0 + 24] {
        let mut b = base.clone();
        b[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(aseprite::read(temp(&b).path()).is_err());
    }
    let mut b = base.clone();
    b[cel.0 + 9..cel.0 + 11].copy_from_slice(&(-1i16).to_le_bytes());
    assert!(aseprite::read(temp(&b).path()).is_err());
    let mut b = base.clone();
    b[cel.0 + 15..cel.0 + 17].copy_from_slice(&1i16.to_le_bytes());
    assert!(aseprite::read(temp(&b).path()).is_err());
    let mut b = base.clone();
    b[cel.0 + 26] ^= 0xff;
    assert!(aseprite::read(temp(&b).path()).is_err());
    assert_eq!(
        aseprite::inspect(temp(&b).path()).unwrap()["raster_validated"],
        false
    );
}

#[test]
fn stale_group_hints_do_not_relink_a_native_cel_edited_to_be_independent() {
    let base = std::fs::read(written(&model()).path()).unwrap();
    let all = chunks(&base);
    let sources: Vec<_> = all
        .iter()
        .filter(|(p, _, ty, _)| *ty == 0x2005 && base[*p + 13] == 2)
        .collect();
    let linked = all
        .iter()
        .find(|(p, _, ty, _)| *ty == 0x2005 && base[*p + 13] == 1)
        .unwrap();
    let source = sources[2];
    let mut b = base.clone();
    b.splice(
        linked.0..linked.0 + linked.1,
        base[source.0..source.0 + source.1].iter().copied(),
    );
    let delta = source.1 - linked.1;
    let n = (base.len() + delta) as u32;
    b[..4].copy_from_slice(&n.to_le_bytes());
    let frame_size =
        u32::from_le_bytes(base[linked.3..linked.3 + 4].try_into().unwrap()) + delta as u32;
    b[linked.3..linked.3 + 4].copy_from_slice(&frame_size.to_le_bytes());
    let s = aseprite::read(temp(&b).path()).unwrap();
    assert!(s.layers[0].timeline[2].link.is_none());
    assert!(s.layers[0].timeline[3].link.is_none());
    assert!(s.layers[0].timeline[4].link.is_some());
}

#[test]
fn raw_rgba_and_forward_links_are_supported() {
    let s = model();
    let base = std::fs::read(written(&s).path()).unwrap();
    let all = chunks(&base);
    let source = all.iter().find(|c| c.2 == 0x2005).unwrap();
    let mut payload = base[source.0 + 6..source.0 + 26].to_vec();
    payload[7..9].copy_from_slice(&0u16.to_le_bytes());
    payload.extend_from_slice(&s.images["paint"]);
    let bytes = replace_chunk(&base, source, &payload);
    let r = aseprite::read(temp(&bytes).path()).unwrap();
    assert_eq!(&*r.images["paint"], &*s.images["paint"]);
    let cs: Vec<_> = all.iter().filter(|c| c.2 == 0x2005).collect();
    let mut bytes = base.clone();
    bytes[cs[3].0 + 22..cs[3].0 + 24].copy_from_slice(&4u16.to_le_bytes());
    // The existing linked frame now explicitly targets a later independent raster cel.
    let r = aseprite::read(temp(&bytes).path()).unwrap();
    assert_eq!(r.layers[0].timeline[3].link, r.layers[0].timeline[4].link);
    assert!(r.layers[0].timeline[2].link.is_none());
}

#[test]
fn decompression_checks_actual_size_trailing_bytes_and_canvas_budget() {
    let s = model();
    let base = std::fs::read(written(&s).path()).unwrap();
    let all = chunks(&base);
    let cel = all.iter().find(|c| c.2 == 0x2005).unwrap();
    for n in [7, 9, 1024] {
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&vec![0; n]).unwrap();
        let mut payload = base[cel.0 + 6..cel.0 + 26].to_vec();
        payload.extend_from_slice(&encoder.finish().unwrap());
        let b = replace_chunk(&base, cel, &payload);
        assert!(
            aseprite::read(temp(&b).path())
                .unwrap_err()
                .to_string()
                .contains("byte count")
        );
    }
    let mut payload = base[cel.0 + 6..cel.0 + cel.1].to_vec();
    payload.extend_from_slice(&[1, 2, 3]);
    let b = replace_chunk(&base, cel, &payload);
    assert!(
        aseprite::read(temp(&b).path())
            .unwrap_err()
            .to_string()
            .contains("trailing data")
    );
    let mut b = base.clone();
    b[8..10].copy_from_slice(&4096u16.to_le_bytes());
    b[10..12].copy_from_slice(&4096u16.to_le_bytes());
    assert!(aseprite::read(temp(&b).path()).is_err());
}

#[test]
fn unrepresentable_metadata_and_orphan_pixels_are_rejected() {
    let mut s = model();
    s.annotations.user_data = "x".repeat(65536);
    assert!(
        aseprite::write(std::io::Cursor::new(Vec::new()), &s)
            .unwrap_err()
            .to_string()
            .contains("65535")
    );
    let mut s = model();
    s.images
        .insert("unused".into(), std::sync::Arc::from([0u8; 8]));
    assert!(
        aseprite::write(std::io::Cursor::new(Vec::new()), &s)
            .unwrap_err()
            .to_string()
            .contains("unused cel")
    );
}

#[test]
fn empty_groups_are_native_transparent_cels_and_native_links() {
    let mut s = model();
    s.layers[0].timeline[0] = schema::Entry {
        cel: None,
        link: Some("blank-group".into()),
        opacity: 0.5,
        annotations: Default::default(),
    };
    s.layers[0].timeline[1] = s.layers[0].timeline[0].clone();
    s.layers[0].timeline[2] = schema::Entry {
        cel: None,
        link: None,
        opacity: 0.3,
        annotations: schema::Annotations {
            user_data: "unlinked blank".into(),
            ..Default::default()
        },
    };
    s.layers[0].timeline[3] = schema::Entry {
        cel: None,
        link: Some("empty-singleton".into()),
        opacity: 0.7,
        annotations: Default::default(),
    };
    let file = written(&s);
    let bytes = std::fs::read(file.path()).unwrap();
    let cel_chunks: Vec<_> = chunks(&bytes)
        .into_iter()
        .filter(|c| c.2 == 0x2005)
        .collect();
    assert_eq!(bytes[cel_chunks[0].0 + 13], 2);
    assert_eq!(bytes[cel_chunks[1].0 + 13], 1);
    assert_eq!(
        u16::from_le_bytes(
            bytes[cel_chunks[1].0 + 22..cel_chunks[1].0 + 24]
                .try_into()
                .unwrap()
        ),
        0
    );
    let r = aseprite::read(file.path()).unwrap();
    assert_eq!(
        serde_json::to_value(&r.layers).unwrap(),
        serde_json::to_value(&s.layers).unwrap()
    );
    for f in 0..s.frames.len() {
        assert_eq!(
            raster::render(&s, f).unwrap(),
            raster::render(&r, f).unwrap()
        );
    }
    // Changing the actual source pixels must defeat stale null provenance for the entire group.
    let source = cel_chunks[0];
    let mut payload = bytes[source.0 + 6..source.0 + 26].to_vec();
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&[255, 0, 0, 255, 0, 0, 0, 0]).unwrap();
    payload.extend_from_slice(&encoder.finish().unwrap());
    let changed = aseprite::read(temp(&replace_chunk(&bytes, &source, &payload)).path()).unwrap();
    assert!(changed.layers[0].timeline[0].cel.is_some());
    assert_eq!(
        changed.layers[0].timeline[0].cel,
        changed.layers[0].timeline[1].cel
    );
    assert_eq!(
        changed.layers[0].timeline[0].link,
        changed.layers[0].timeline[1].link
    );
}

#[test]
fn native_user_colors_and_palette_bytes_survive_an_authored_roundtrip() {
    let mut s = model();
    s.layers[0]
        .annotations
        .metadata
        .insert("aseprite_native".into(), json!({"color":"01020304"}));
    s.layers[0].timeline[0]
        .annotations
        .metadata
        .insert("aseprite_native".into(), json!({"color":"aabbccdd"}));
    let r = aseprite::read(written(&s).path()).unwrap();
    assert_eq!(
        r.layers[0].annotations.metadata["aseprite_native"],
        s.layers[0].annotations.metadata["aseprite_native"]
    );
    assert_eq!(
        r.layers[0].timeline[0].annotations.metadata["aseprite_native"],
        s.layers[0].timeline[0].annotations.metadata["aseprite_native"]
    );
}

#[test]
fn unsupported_tag_semantics_user_properties_and_visual_chunks_are_explicit() {
    let base = std::fs::read(written(&model()).path()).unwrap();
    let all = chunks(&base);
    let tag = all.iter().find(|c| c.2 == 0x2018).unwrap();
    for (offset, value) in [(tag.0 + 20, 1u8), (tag.0 + 21, 1u8)] {
        let mut b = base.clone();
        b[offset] = value;
        let f = temp(&b);
        assert!(
            aseprite::read(f.path())
                .unwrap_err()
                .to_string()
                .contains("only forward")
        );
        assert!(
            !aseprite::inspect(f.path()).unwrap()["unsupported_features"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    let user = all.iter().find(|c| c.2 == 0x2020).unwrap();
    let mut b = base.clone();
    b[user.0 + 6] = 5;
    assert!(aseprite::read(temp(&b).path()).is_err());
    let cel = all.iter().find(|c| c.2 == 0x2005).unwrap();
    for kind in [0x2006u16, 0x2022, 0xffff] {
        let mut b = base.clone();
        b[cel.0 + 4..cel.0 + 6].copy_from_slice(&kind.to_le_bytes());
        assert!(aseprite::read(temp(&b).path()).is_err());
        assert!(
            !aseprite::inspect(temp(&b).path()).unwrap()["unsupported_features"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn writer_preflights_native_raster_residency_without_inventing_links() {
    let mut s = model();
    s.width = 1024;
    s.height = 1024;
    s.images.clear();
    s.images.insert(
        "paint".into(),
        std::sync::Arc::from(vec![0u8; 1024 * 1024 * 4]),
    );
    s.frames = vec![
        schema::Frame {
            duration_ms: 100.0,
            annotations: Default::default()
        };
        65
    ];
    s.layers[0].timeline = vec![
        schema::Entry {
            cel: Some("paint".into()),
            link: None,
            opacity: 1.0,
            annotations: Default::default()
        };
        65
    ];
    s.tags.clear();
    let mut output = std::io::Cursor::new(Vec::new());
    let error = aseprite::write(&mut output, &s).unwrap_err();
    assert!(error.to_string().contains("resident buffer limit"));
    assert!(
        output.into_inner().is_empty(),
        "budget rejection must occur before writing any output bytes"
    );
    // Explicit links are the only way these occurrences share a native editing source.
    for entry in &mut s.layers[0].timeline {
        entry.link = Some("held".into());
    }
    let f = written(&s);
    let bytes = std::fs::read(f.path()).unwrap();
    let types: Vec<_> = chunks(&bytes)
        .into_iter()
        .filter(|c| c.2 == 0x2005)
        .map(|c| bytes[c.0 + 13])
        .collect();
    assert_eq!(types.iter().filter(|&&ty| ty == 2).count(), 1);
    assert_eq!(types.iter().filter(|&&ty| ty == 1).count(), 64);
    let r = aseprite::read(f.path()).unwrap();
    assert_eq!(r.frames.len(), 65);
    assert_eq!(r.images.len(), 1);
    assert_eq!(r.layers[0].timeline[0].link, r.layers[0].timeline[64].link);
}

#[test]
fn legacy_spritex_metadata_envelopes_remain_readable() {
    let expected = model();
    let file = written(&expected);
    let expected = aseprite::read(file.path()).unwrap();
    let bytes = std::fs::read(file.path()).unwrap();
    let old = b"spritex-aseprite-v1:";
    let new = b"pixloom-aseprite-v1:";
    let mut legacy = bytes.clone();
    let mut count = 0;
    for i in 0..=bytes.len() - new.len() {
        if &bytes[i..i + new.len()] == new {
            legacy[i..i + old.len()].copy_from_slice(old);
            count += 1;
        }
    }
    assert!(count > 0);
    let file = temp(&legacy);
    let actual = aseprite::read(file.path()).unwrap();
    assert_eq!(
        serde_json::to_value(actual.annotations).unwrap(),
        serde_json::to_value(expected.annotations).unwrap()
    );
    assert_eq!(actual.frames.len(), expected.frames.len());
}
