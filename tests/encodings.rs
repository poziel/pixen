use pixloom::schema;
use serde_json::{Value, json};
fn doc(cel: Value) -> Value {
    json!({"version":2,"canvas":{"width":3,"height":2},"palette":["#FF0000FF","#12345600","#FF0000FF"],"cels":[cel],"frames":[{"duration_ms":100}],"layers":[{"id":"l","name":"L","timeline":[{"cel":"c"}]}]})
}
#[test]
fn five_explicit_encodings_render_identically() {
    let cels = [
        json!({"id":"c","encoding":"grid","pixels":[[0,2,2],[1,1,0]]}),
        json!({"id":"c","encoding":"hybrid-rle","rows":[[0,[2,2]],[[1,2],0]]}),
        json!({"id":"c","encoding":"sparse-rows","default_index":1,"rows":[{"y":0,"spans":[{"x":0,"pixels":[0,2,2]}]},{"y":1,"spans":[{"x":2,"pixels":[0]}]}]}),
        json!({"id":"c","encoding":"row-table","row_definitions":[[0,2,2],[1,1,0]],"rows":[0,1]}),
        json!({"id":"c","encoding":"tiles","tile_size":[2,2],"default_index":1,"tiles":[[[0,2],[1,1]],[[2,1],[0,1]]],"map":[[0,1]]}),
    ];
    for cel in cels {
        let s = schema::parse(&serde_json::to_vec(&doc(cel)).unwrap()).unwrap();
        assert_eq!(
            &*s.images["c"],
            &[
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 18, 52, 86, 0, 18, 52, 86, 0, 255,
                0, 0, 255
            ]
        );
    }
}

#[test]
fn exact_index_writer_round_trips_randomized_full_and_cropped_images() {
    use pixloom::encoding::{self, Kind, Placement};
    let kinds = [
        Kind::Grid,
        Kind::HybridRle,
        Kind::SparseRows,
        Kind::RowTable,
        Kind::Tiles { tile_size: [3, 2] },
    ];
    let mut seed = 0x12345678u32;
    for w in 1..=9 {
        for h in 1..=7 {
            for cropped in [false, true] {
                for kind in kinds {
                    let canvas = schema::Canvas {
                        width: w,
                        height: h,
                    };
                    let (x, y, rw, rh) = if cropped && w > 2 && h > 2 {
                        (1, 1, w - 2, h - 2)
                    } else {
                        (0, 0, w, h)
                    };
                    let is_crop = rw != w || rh != h;
                    let needs_default = is_crop
                        || matches!(kind, Kind::SparseRows)
                        || matches!(kind, Kind::Tiles { .. }) && (rw % 3 != 0 || rh % 2 != 0);
                    let p = Placement {
                        offset: cropped.then_some([x, y]),
                        size: cropped.then_some([rw, rh]),
                        default_index: needs_default.then_some(1),
                    };
                    let mut image = vec![vec![1; w as usize]; h as usize];
                    for row in &mut image[y as usize..(y + rh) as usize] {
                        for pixel in &mut row[x as usize..(x + rw) as usize] {
                            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                            *pixel = (seed >> 16) % 3;
                        }
                    }
                    let encoded =
                        encoding::encode("c".into(), &image, &canvas, 3, kind, p).unwrap();
                    let bytes = serde_json::to_vec(&encoded).unwrap();
                    let reread: encoding::Encoding = serde_json::from_slice(&bytes).unwrap();
                    assert_eq!(
                        reread.decode(&canvas, 3).unwrap(),
                        image,
                        "{kind:?}, {w}x{h}"
                    );
                }
            }
        }
    }
}

#[test]
fn authoring_read_write_preserves_duplicate_palette_entries_and_indices() {
    let v = doc(json!({"id":"c","encoding":"grid","pixels":[[0,2,0],[2,1,2]]}));
    let d = schema::read_document(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert_eq!(
        schema::decode_indices(&d).unwrap()["c"],
        vec![vec![0, 2, 0], vec![2, 1, 2]]
    );
    let mut bytes = Vec::new();
    pixloom::output::write_document(&mut bytes, &d).unwrap();
    let d = schema::read_document(&bytes).unwrap();
    assert_eq!(d.palette, vec!["#FF0000FF", "#12345600", "#FF0000FF"]);
    assert_eq!(
        schema::decode_indices(&d).unwrap()["c"],
        vec![vec![0, 2, 0], vec![2, 1, 2]]
    );
}

#[test]
fn null_placement_and_default_are_invalid_not_missing() {
    for field in ["offset", "size", "default_index"] {
        let mut cel = json!({"id":"c","encoding":"grid","pixels":[[0,0,0],[0,0,0]]});
        cel[field] = Value::Null;
        assert!(
            schema::parse(&serde_json::to_vec(&doc(cel)).unwrap()).is_err(),
            "{field}"
        );
    }
}

fn parse_cel(cel: Value) -> anyhow::Result<pixloom::model::Sprite> {
    schema::parse(&serde_json::to_vec(&doc(cel)).unwrap())
}
#[test]
fn malformed_payloads_fail_safely() {
    let bad = [
        json!({"encoding":"rle","rows":[]}),
        json!({"encoding":"grid","pixels":[[0,0,0],[0,0,0]],"rows":[]}),
        json!({"encoding":"grid","pixels":[[0,0],[0,0]]}),
        json!({"encoding":"grid","pixels":[[0,0,0]]}),
        json!({"encoding":"grid","pixels":[[[0,3]],[[0,3]]]}),
        json!({"encoding":"grid","pixels":[[3,0,0],[0,0,0]]}),
        json!({"encoding":"grid","default_index":0,"pixels":[[0,0,0],[0,0,0]]}),
        json!({"encoding":"grid","offset":[0,0],"pixels":[[0,0,0],[0,0,0]]}),
        json!({"encoding":"grid","size":[3,2],"pixels":[[0,0,0],[0,0,0]]}),
        json!({"encoding":"grid","offset":[0,0],"size":[0,1],"default_index":0,"pixels":[]}),
        json!({"encoding":"grid","offset":[4294967295u32,0],"size":[2,1],"default_index":0,"pixels":[[0,0]]}),
        json!({"encoding":"grid","offset":[2,1],"size":[2,1],"default_index":0,"pixels":[[0,0]]}),
        json!({"encoding":"grid","offset":[1,1],"size":[1,1],"pixels":[[0]]}),
        json!({"encoding":"grid","offset":[1,1],"size":[1,1],"default_index":3,"pixels":[[0]]}),
        json!({"encoding":"hybrid-rle","rows":[[0,0,0]]}),
        json!({"encoding":"hybrid-rle","rows":[[[0,2]],[[0,3]]]}),
        json!({"encoding":"hybrid-rle","rows":[[[0,4]],[[0,3]]]}),
        json!({"encoding":"hybrid-rle","rows":[[[0,4294967295u32]],[[0,3]]]}),
        json!({"encoding":"hybrid-rle","rows":[[[0,0],0,0,0],[[0,3]]]}),
        json!({"encoding":"sparse-rows","rows":[]}),
        json!({"encoding":"sparse-rows","default_index":0,"rows":[{"y":0,"spans":[]}]}),
        json!({"encoding":"sparse-rows","default_index":0,"rows":[{"y":2,"spans":[{"x":0,"pixels":[1]}]}]}),
        json!({"encoding":"sparse-rows","default_index":0,"rows":[{"y":0,"spans":[{"x":0,"pixels":[1]}]},{"y":0,"spans":[{"x":1,"pixels":[1]}]}]}),
        json!({"encoding":"sparse-rows","default_index":0,"rows":[{"y":1,"spans":[{"x":0,"pixels":[1]}]},{"y":0,"spans":[{"x":1,"pixels":[1]}]}]}),
        json!({"encoding":"sparse-rows","default_index":0,"rows":[{"y":0,"spans":[{"x":0,"pixels":[]}]}]}),
        json!({"encoding":"sparse-rows","default_index":0,"rows":[{"y":0,"spans":[{"x":3,"pixels":[1]}]}]}),
        json!({"encoding":"sparse-rows","default_index":0,"rows":[{"y":0,"spans":[{"x":2,"pixels":[1,1]}]}]}),
        json!({"encoding":"sparse-rows","default_index":0,"rows":[{"y":0,"spans":[{"x":0,"pixels":[1,1]},{"x":1,"pixels":[1]}]}]}),
        json!({"encoding":"sparse-rows","default_index":0,"rows":[{"y":0,"spans":[{"x":1,"pixels":[1]},{"x":0,"pixels":[1]}]}]}),
        json!({"encoding":"row-table","row_definitions":[[0,0]],"rows":[0,0]}),
        json!({"encoding":"row-table","row_definitions":[[0,0,0]],"rows":[0]}),
        json!({"encoding":"row-table","row_definitions":[[0,0,0]],"rows":[0,1]}),
        json!({"encoding":"row-table","row_definitions":[[0,0,0],[3,3,3]],"rows":[0,0]}),
        json!({"encoding":"tiles","tile_size":[0,1],"tiles":[],"map":[]}),
        json!({"encoding":"tiles","tile_size":[4294967295u32,4294967295u32],"tiles":[],"map":[]}),
        json!({"encoding":"tiles","tile_size":[2,2],"tiles":[[[0,0],[0,0]]],"map":[[0,0]]}),
        json!({"encoding":"tiles","tile_size":[2,2],"default_index":1,"tiles":[[[0,0],[0,0]]],"map":[[0,0]]}),
        json!({"encoding":"tiles","tile_size":[3,2],"default_index":0,"tiles":[[[0,0,0],[0,0,0]]],"map":[[0]]}),
        json!({"encoding":"tiles","tile_size":[3,2],"tiles":[[[0,0,0]]],"map":[[0]]}),
        json!({"encoding":"tiles","tile_size":[3,2],"tiles":[[[0,0],[0,0]]],"map":[[0]]}),
        json!({"encoding":"tiles","tile_size":[3,2],"tiles":[[[0,0,0],[0,0,0]]],"map":[[1]]}),
        json!({"encoding":"tiles","tile_size":[3,2],"tiles":[[[0,0,0],[0,0,0]]],"map":[]}),
        json!({"encoding":"tiles","tile_size":[3,2],"tiles":[[[0,0,0],[0,0,0]]],"map":[[0,0]]}),
    ];
    for (n, mut cel) in bad.into_iter().enumerate() {
        cel["id"] = json!("c");
        assert!(parse_cel(cel.clone()).is_err(), "case {n}: {cel}");
    }
    for token in [
        json!(-1),
        json!(0.5),
        json!(4294967296u64),
        json!([]),
        json!([1]),
        json!([1, 2, 3]),
        json!([[1, 2], 3]),
        json!([0, -1]),
        json!([0, 1.5]),
        json!(null),
        json!("0"),
    ] {
        assert!(
            parse_cel(json!({"id":"c","encoding":"hybrid-rle","rows":[[token,0,0],[[0,3]]]}))
                .is_err()
        );
    }
}

#[test]
fn canonical_tokens_use_strict_json_savings_and_accept_noncanonical_runs() {
    use pixloom::encoding::hybrid::{Token, encode};
    assert_eq!(encode(&[0, 0, 0]), vec![Token::Literal(0); 3]); // [0,3] ties 0,0,0
    assert_eq!(encode(&[0, 0, 0, 0]), vec![Token::Run([0, 4])]);
    assert_eq!(encode(&[123, 123]), vec![Token::Literal(123); 2]); // tie
    assert_eq!(encode(&[1234, 1234]), vec![Token::Run([1234, 2])]);
    assert!(
        parse_cel(json!({"id":"c","encoding":"hybrid-rle","rows":[[[0,1],2,[2,1]],[[1,1],1,0]]}))
            .is_ok()
    );
}

#[test]
fn defaults_small_positioned_grids_sparse_adjacency_and_all_default() {
    let s=parse_cel(json!({"id":"c","encoding":"grid","offset":[1,1],"size":[1,1],"default_index":1,"pixels":[[0]]})).unwrap();
    assert_eq!(
        &*s.images["c"],
        &[
            18, 52, 86, 0, 18, 52, 86, 0, 18, 52, 86, 0, 18, 52, 86, 0, 255, 0, 0, 255, 18, 52, 86,
            0
        ]
    );
    let c: pixloom::encoding::Encoding = serde_json::from_value(
        json!({"id":"c","encoding":"sparse-rows","default_index":2,"rows":[]}),
    )
    .unwrap();
    assert_eq!(
        c.decode(
            &schema::Canvas {
                width: 3,
                height: 2
            },
            3
        )
        .unwrap(),
        vec![vec![2; 3]; 2]
    );
    assert!(parse_cel(json!({"id":"c","encoding":"sparse-rows","default_index":1,"rows":[{"y":0,"spans":[{"x":0,"pixels":[1]},{"x":1,"pixels":[0,1]}]}]})).is_ok());
}

#[test]
fn dictionaries_deduplicate_by_indices_in_first_occurrence_order() {
    use pixloom::encoding::{self, Kind, Placement};
    let image = vec![vec![2, 0], vec![0, 2], vec![2, 0]];
    let canvas = schema::Canvas {
        width: 2,
        height: 3,
    };
    let e = encoding::encode(
        "c".into(),
        &image,
        &canvas,
        3,
        Kind::RowTable,
        Placement::default(),
    )
    .unwrap();
    let v = serde_json::to_value(e).unwrap();
    assert_eq!(v["row_definitions"], json!([[2, 0], [0, 2]]));
    assert_eq!(v["rows"], json!([0, 1, 0]));
    let e = encoding::encode(
        "c".into(),
        &image,
        &canvas,
        3,
        Kind::Tiles { tile_size: [2, 1] },
        Placement::default(),
    )
    .unwrap();
    let v = serde_json::to_value(e).unwrap();
    assert_eq!(v["tiles"], json!([[[2, 0]], [[0, 2]]]));
    assert_eq!(v["map"], json!([[0], [1], [0]]));
    // Duplicated/unused definitions are valid, but must themselves be valid.
    assert!(parse_cel(json!({"id":"c","encoding":"row-table","row_definitions":[[0,0,0],[0,0,0],[2,2,2]],"rows":[0,1]})).is_ok());
}

#[test]
fn versions_are_explicit_and_legacy_remains_readable() {
    let mut v = doc(json!({"id":"c","pixels":[[0,2,0],[1,1,1]]}));
    assert!(schema::parse(&serde_json::to_vec(&v).unwrap()).is_err());
    v["version"] = json!(1);
    assert!(schema::parse(&serde_json::to_vec(&v).unwrap()).is_ok());
    v["cels"][0]["encoding"] = json!("grid");
    assert!(schema::parse(&serde_json::to_vec(&v).unwrap()).is_err());
    v["version"] = json!(3);
    assert!(schema::parse(&serde_json::to_vec(&v).unwrap()).is_err());
}

#[test]
fn mixed_encodings_preserve_timelines_and_native_rendering_without_implicit_links() {
    use pixloom::{
        aseprite,
        encoding::{self, Kind, Placement},
        pxo, raster,
    };
    let image = vec![vec![0, 2, 2], vec![1, 1, 0]];
    let canvas = schema::Canvas {
        width: 3,
        height: 2,
    };
    let kinds = [
        Kind::Grid,
        Kind::HybridRle,
        Kind::SparseRows,
        Kind::RowTable,
        Kind::Tiles { tile_size: [2, 2] },
    ];
    let cels: Vec<_> = kinds
        .into_iter()
        .enumerate()
        .map(|(n, kind)| {
            let p = Placement {
                default_index: matches!(kind, Kind::SparseRows | Kind::Tiles { .. }).then_some(1),
                ..Default::default()
            };
            serde_json::to_value(
                encoding::encode(format!("c{n}"), &image, &canvas, 3, kind, p).unwrap(),
            )
            .unwrap()
        })
        .collect();
    let mut v = doc(cels[0].clone());
    v["cels"] = json!(cels);
    v["frames"] = json!([{"duration_ms":100},{"duration_ms":100},{"duration_ms":100}]);
    v["layers"] = json!([
 {"id":"a","name":"A","timeline":[{"cel":"c0"},{"cel":"c1"},{"cel":"c2"}]},
 {"id":"b","name":"B","timeline":[{"cel":"c3"},{"cel":"c4"},{"cel":"c3"}]}]);
    let s = schema::parse(&serde_json::to_vec(&v).unwrap()).unwrap();
    assert!(
        s.layers
            .iter()
            .flat_map(|l| &l.timeline)
            .all(|e| e.link.is_none())
    );
    for native in [false, true] {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        if native {
            aseprite::write(&mut file, &s).unwrap();
        } else {
            pxo::write(&mut file, &s).unwrap();
        }
        let reread = if native {
            aseprite::read(file.path()).unwrap()
        } else {
            pxo::read(file.path()).unwrap()
        };
        for f in 0..3 {
            assert_eq!(
                raster::render(&s, f).unwrap(),
                raster::render(&reread, f).unwrap()
            );
        }
        assert!(
            reread
                .layers
                .iter()
                .flat_map(|l| &l.timeline)
                .all(|e| e.link.is_none())
        );
    }
}

#[test]
fn unused_dictionary_pixels_and_definitions_are_budgeted_before_expansion() {
    let mut v = doc(
        json!({"id":"c","encoding":"row-table","row_definitions":vec![json!([[0,4096]]);16385],"rows":[0]}),
    );
    v["canvas"] = json!({"width":4096,"height":1});
    assert!(
        schema::parse(&serde_json::to_vec(&v).unwrap())
            .unwrap_err()
            .to_string()
            .contains("dictionary pixels")
    );
    let mut v = doc(
        json!({"id":"c","encoding":"row-table","row_definitions":vec![json!([0,0,0]);32769],"rows":[0,0]}),
    );
    assert!(schema::parse(&serde_json::to_vec(&v).unwrap()).is_err());
    v["cels"] = json!([
 {"id":"c","encoding":"row-table","row_definitions":vec![json!([0,0,0]);16385],"rows":[0,0]},
 {"id":"d","encoding":"row-table","row_definitions":vec![json!([0,0,0]);16385],"rows":[0,0]}]);
    assert!(
        schema::parse(&serde_json::to_vec(&v).unwrap())
            .unwrap_err()
            .to_string()
            .contains("dictionary definitions")
    );
}

#[test]
fn all_default_images_and_maximum_width_decode_in_every_encoding() {
    use pixloom::encoding::{self, Kind, Placement};
    for kind in [
        Kind::Grid,
        Kind::HybridRle,
        Kind::SparseRows,
        Kind::RowTable,
        Kind::Tiles { tile_size: [3, 2] },
    ] {
        for (w, h) in [(1, 1), (4, 3), (4096, 1)] {
            let canvas = schema::Canvas {
                width: w,
                height: h,
            };
            let image = vec![vec![2; w as usize]; h as usize];
            let needs = matches!(kind, Kind::SparseRows)
                || matches!(kind, Kind::Tiles { .. }) && (w % 3 != 0 || h % 2 != 0);
            let e = encoding::encode(
                "c".into(),
                &image,
                &canvas,
                3,
                kind,
                Placement {
                    default_index: needs.then_some(2),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(e.decode(&canvas, 3).unwrap(), image);
            if matches!(kind, Kind::SparseRows) {
                assert_eq!(serde_json::to_value(e).unwrap()["rows"], json!([]));
            }
        }
    }
}

#[test]
fn each_encoding_rejects_foreign_fields_and_duplicate_fields() {
    let valid = [
        json!({"id":"c","encoding":"grid","pixels":[[0,0,0],[0,0,0]]}),
        json!({"id":"c","encoding":"hybrid-rle","rows":[[[0,3]],[[0,3]]]}),
        json!({"id":"c","encoding":"sparse-rows","default_index":1,"rows":[]}),
        json!({"id":"c","encoding":"row-table","row_definitions":[[[0,3]]],"rows":[0,0]}),
        json!({"id":"c","encoding":"tiles","tile_size":[3,2],"tiles":[[[[0,3]],[[0,3]]]],"map":[[0]]}),
    ];
    for mut cel in valid {
        let raw = serde_json::to_string(&doc(cel.clone()))
            .unwrap()
            .replace("\"id\":\"c\"", "\"id\":\"c\",\"id\":\"c\"");
        assert!(schema::parse(raw.as_bytes()).is_err());
        cel["length"] = json!(1);
        assert!(parse_cel(cel).is_err());
    }
}

#[test]
fn tile_padding_is_checked_for_every_boundary_placement_and_unused_tiles() {
    // A shared tile valid at the left is invalid at the right when its last pixel is not default.
    assert!(parse_cel(json!({"id":"c","encoding":"tiles","tile_size":[2,2],"default_index":1,"tiles":[[[0,2],[1,0]]],"map":[[0,0]]})).is_err());
    // Vertical padding as well as horizontal padding must be default.
    let mut v = doc(
        json!({"id":"c","encoding":"tiles","tile_size":[3,2],"default_index":1,"tiles":[[[0,0,0],[2,2,2]]],"map":[[0]]}),
    );
    v["canvas"]["height"] = json!(1);
    assert!(schema::parse(&serde_json::to_vec(&v).unwrap()).is_err());
    assert!(parse_cel(json!({"id":"c","encoding":"tiles","tile_size":[3,2],"tiles":[[[0,0,0],[0,0,0]],[[3,3,3],[3,3,3]]],"map":[[0]]})).is_err());
    let mut v = doc(json!({"id":"c","encoding":"sparse-rows","default_index":1,"rows":[]}));
    v["canvas"] = json!({"width":4096,"height":1024});
    v["cels"]=json!((0..17).map(|i| json!({"id":format!("c{i}"),"encoding":"sparse-rows","default_index":1,"rows":[]})).collect::<Vec<_>>());
    assert!(
        schema::parse(&serde_json::to_vec(&v).unwrap())
            .unwrap_err()
            .to_string()
            .contains("cel grids")
    );
}
