use pixloom::{raster, schema};
use serde_json::{Value, json};

fn document() -> Value {
    json!({"version":1,"canvas":{"width":2,"height":1},
      "palette":["#FF0000FF","#12345600","#0000FF80"],
      "cels":[{"id":"base","pixels":[[0,1]]},{"id":"top","pixels":[[2,2]]}],
      "frames":[{"duration_ms":125}],
      "layers":[{"id":"base","name":"Base","opacity":1,"timeline":[{"cel":"base","opacity":1}]}]})
}
fn parse(v: Value) -> anyhow::Result<pixloom::model::Sprite> {
    schema::parse(&serde_json::to_vec(&v)?)
}

#[test]
fn exact_rgba_and_transparency_is_not_index_zero() {
    assert_eq!(
        schema::decode_color("#aB012380").unwrap(),
        [171, 1, 35, 128]
    );
    for s in ["#123456", "12345678", "#GG000000", "#é000000", "#000000000"] {
        assert!(schema::decode_color(s).is_err());
    }
    let sprite = parse(document()).unwrap();
    assert_eq!(&*sprite.images["base"], &[255, 0, 0, 255, 18, 52, 86, 0]);
    assert_eq!(
        raster::render(&sprite, 0).unwrap(),
        [255, 0, 0, 255, 0, 0, 0, 0]
    );
}
#[test]
fn source_over_layer_order_visibility_and_opacity() {
    let mut v = document();
    v["layers"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"top","name":"Top","timeline":[{"cel":"top"}]}));
    assert_eq!(
        raster::render(&parse(v.clone()).unwrap(), 0).unwrap(),
        [127, 0, 128, 255, 0, 0, 255, 128]
    );
    v["layers"][1]["opacity"] = json!(0.5);
    v["layers"][1]["timeline"][0]["opacity"] = json!(0.5);
    assert_eq!(
        raster::render(&parse(v.clone()).unwrap(), 0).unwrap(),
        [223, 0, 32, 255, 0, 0, 255, 32]
    );
    v["layers"][1]["visible"] = json!(false);
    assert_eq!(
        raster::render(&parse(v).unwrap(), 0).unwrap(),
        [255, 0, 0, 255, 0, 0, 0, 0]
    );
}
#[test]
fn validates_malformed_inputs() {
    let mutations: Vec<(&str, Value)> = vec![
        ("/version", json!(2)),
        ("/canvas/width", json!(0)),
        ("/canvas/height", json!(4294967295u32)),
        ("/cels/0/pixels", json!([[0]])),
        ("/cels/0/pixels/0/0", json!(3)),
        ("/cels/1/id", json!("base")),
        ("/layers/0/id", json!("bad/id")),
        ("/layers/0/timeline/0/cel", json!("missing")),
        ("/layers/0/timeline", json!([])),
        ("/layers/0/opacity", json!(1.1)),
        ("/layers/0/timeline/0/opacity", json!(-0.1)),
        ("/frames/0/duration_ms", json!(0)),
        ("/fps", json!(0)),
    ];
    for (pointer, value) in mutations {
        let mut v = document();
        if pointer == "/fps" {
            v["fps"] = value
        } else {
            *v.pointer_mut(pointer).unwrap() = value
        };
        assert!(parse(v).is_err(), "{pointer}");
    }
    let mut v = document();
    v["unknown"] = json!(true);
    assert!(parse(v).is_err());
    let mut v = document();
    v["tags"] = json!([{"name":"bad","from":0,"to":1}]);
    assert!(parse(v).is_err());
}
#[test]
fn reused_grids_are_independent_unless_explicitly_linked() {
    let mut v = document();
    v["frames"] = json!([{"duration_ms":125},{"duration_ms":250},{"duration_ms":125}]);
    v["layers"][0]["timeline"] =
        json!([{"cel":"base","link":"hold"},{"cel":"base"},{"cel":"base","link":"hold"}]);
    let s = parse(v.clone()).unwrap();
    assert_eq!(s.frames[1].duration_ms, 250.0);
    assert!(s.layers[0].timeline[1].link.is_none());
    v["layers"][0]["timeline"][2]["cel"] = json!("top");
    assert!(parse(v).is_err());
}
#[test]
fn palette_indices_can_exceed_255_and_empty_cels_are_explicit() {
    let mut v = document();
    v["palette"] = json!(vec!["#FF00FFFF"; 300]);
    v["cels"][0]["pixels"] = json!([[299, 299]]);
    assert_eq!(
        &*parse(v.clone()).unwrap().images["base"],
        &[255, 0, 255, 255, 255, 0, 255, 255]
    );
    v["layers"][0]["timeline"][0]["cel"] = Value::Null;
    assert_eq!(raster::render(&parse(v).unwrap(), 0).unwrap(), vec![0; 8]);
}
