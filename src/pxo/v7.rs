use crate::{
    archive::Archive,
    model::Sprite,
    raster,
    schema::{self, Annotations, Entry, Frame, Layer, Tag},
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    hash::{Hash, Hasher},
    io::{Seek, Write},
    path::Path,
    sync::Arc,
};

#[derive(Deserialize)]
struct Header {
    #[serde(flatten)]
    pixelorama: schema::PixeloramaExtras,
    pxo_version: u32,
    color_mode: i32,
    size_x: u32,
    size_y: u32,
    fps: f64,
    layers: Vec<RawLayer>,
    frames: Vec<RawFrame>,
    #[serde(default)]
    tags: Vec<Tag>,
    #[serde(default)]
    metadata: BTreeMap<String, Value>,
    #[serde(default)]
    user_data: String,
}
#[derive(Deserialize)]
struct RawLayer {
    name: String,
    #[serde(default = "yes")]
    visible: bool,
    #[serde(default = "one")]
    opacity: f64,
    #[serde(default)]
    locked: bool,
    #[serde(default)]
    new_cels_linked: bool,
    #[serde(default)]
    link_sets: Vec<LinkSet>,
    #[serde(default)]
    metadata: BTreeMap<String, Value>,
    #[serde(default)]
    user_data: String,
}
#[derive(Deserialize)]
struct LinkSet {
    cels: Vec<usize>,
}
#[derive(Deserialize)]
struct RawFrame {
    duration: f64,
    cels: Vec<RawCel>,
    #[serde(default)]
    metadata: BTreeMap<String, Value>,
    #[serde(default)]
    user_data: String,
}
#[derive(Deserialize)]
struct RawCel {
    opacity: f64,
    #[serde(default)]
    metadata: BTreeMap<String, Value>,
    #[serde(default)]
    user_data: String,
}
fn yes() -> bool {
    true
}
fn one() -> f64 {
    1.0
}

fn load(path: &Path) -> Result<(Archive, Value)> {
    let mut archive = Archive::open(path)?;
    ensure!(
        archive.read("mimetype", 24)? == b"application/x-pixelorama",
        "invalid PXO mimetype"
    );
    let bytes = archive.read("data.json", 16 * 1024 * 1024)?;
    let mut data: Value = serde_json::from_slice(&bytes).context("invalid PXO data.json")?;
    ensure!(data.is_object(), "PXO data.json must be an object");
    // Native tag colours omit '#'; authoring colours always include it.
    if let Some(tags) = data.get_mut("tags").and_then(Value::as_array_mut) {
        for tag in tags {
            if let Some(color) = tag.get_mut("color") {
                if let Some(s) = color.as_str() {
                    if !s.starts_with('#') {
                        *color = json!(format!("#{s}"));
                    }
                }
            }
        }
    }
    Ok((archive, data))
}
fn nonempty(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        _ => true,
    }
}
fn unknown_keys(v: &Value, known: &[&str], where_: &str, out: &mut Vec<String>) {
    if let Some(o) = v.as_object() {
        for key in o.keys() {
            if !known.contains(&key.as_str()) {
                out.push(format!("{where_}: unknown field {key}"));
            }
        }
    }
}
fn unsupported(d: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if d["pxo_version"] != 7 {
        out.push("only PXO version 7 is supported".into());
    }
    if d["color_mode"] != 5 {
        out.push("only RGBA8 color_mode 5 is supported".into());
    }
    for key in [
        "brushes",
        "reference_images",
        "tilesets",
        "guides",
        "vanishing_points",
    ] {
        if d.get(key).is_some_and(nonempty) {
            out.push(format!("project {key} are unsupported"));
        }
    }
    unknown_keys(
        d,
        &[
            "pixelorama_version",
            "pxo_version",
            "size_x",
            "size_y",
            "color_mode",
            "tile_mode_x_basis_x",
            "tile_mode_x_basis_y",
            "tile_mode_y_basis_x",
            "tile_mode_y_basis_y",
            "layers",
            "next_keyframe_id",
            "tags",
            "guides",
            "symmetry_points",
            "frames",
            "current_frame",
            "current_layer",
            "brushes",
            "palettes",
            "project_current_palette_name",
            "reference_images",
            "tilesets",
            "vanishing_points",
            "fps",
            "license",
            "user_data",
            "author_display_name",
            "author_real_name",
            "author_contact",
            "author_company",
            "metadata",
            "export_profile",
        ],
        "project",
        &mut out,
    );
    if let Some(layers) = d["layers"].as_array() {
        for (i, l) in layers.iter().enumerate() {
            let label = format!("layer {}", i + 1);
            for (key, expected) in [
                ("type", json!(0)),
                ("blend_mode", json!(0)),
                ("parent", json!(-1)),
                ("clipping_mask", json!(false)),
            ] {
                if l.get(key).is_some_and(|v| v != &expected)
                    || (key == "type" && l.get(key).is_none())
                {
                    out.push(format!("{label}: unsupported {key}"));
                }
            }
            if l.get("effects").is_some_and(nonempty) {
                out.push(format!("{label}: effects are unsupported"));
            }
            if let Some(params) = l.get("animated_params") {
                let empty = if let Some(s) = params.as_str() {
                    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
                    [
                        "{}",
                        "{\"opacity\":{}}",
                        "Dictionary[String,Dictionary]({\"opacity\":{}})",
                    ]
                    .contains(&compact.as_str())
                } else {
                    params == &json!({}) || params == &json!({"opacity":{}})
                };
                if !empty {
                    out.push(format!("{label}: animated parameters are unsupported"));
                }
            }
            unknown_keys(
                l,
                &[
                    "name",
                    "visible",
                    "locked",
                    "blend_mode",
                    "clipping_mask",
                    "opacity",
                    "ui_color",
                    "parent",
                    "effects",
                    "animated_params",
                    "user_data",
                    "link_sets",
                    "new_cels_linked",
                    "type",
                    "metadata",
                ],
                &label,
                &mut out,
            );
        }
    }
    if let Some(frames) = d["frames"].as_array() {
        for (f, frame) in frames.iter().enumerate() {
            unknown_keys(
                frame,
                &["cels", "duration", "metadata", "user_data"],
                &format!("frame {}", f + 1),
                &mut out,
            );
            if let Some(cels) = frame["cels"].as_array() {
                for (l, cel) in cels.iter().enumerate() {
                    if cel.get("z_index").is_some_and(|v| v != 0) {
                        out.push(format!(
                            "frame {} layer {}: nonzero z_index is unsupported",
                            f + 1,
                            l + 1
                        ));
                    }
                    unknown_keys(
                        cel,
                        &["opacity", "z_index", "user_data", "ui_color", "metadata"],
                        &format!("frame {} cel {}", f + 1, l + 1),
                        &mut out,
                    );
                }
            }
        }
    }
    out
}

pub fn inspect(path: &Path) -> Result<Value> {
    let (_archive, d) = load(path)?;
    let issues = unsupported(&d);
    let frames = d["frames"].as_array().context("missing frames array")?;
    let layers = d["layers"].as_array().context("missing layers array")?;
    let fps = d["fps"].as_f64().context("invalid fps")?;
    schema::positive(fps, "fps")?;
    let timing: Vec<_> = frames
        .iter()
        .map(|f| f["duration"].as_f64().map(|x| x / fps * 1000.0))
        .collect();
    Ok(
        json!({"target":"Pixelorama v1.2.3-stable / PXO 7","pxo_version":d["pxo_version"],"pixelorama_version":d["pixelorama_version"],"width":d["size_x"],"height":d["size_y"],"color_mode":d["color_mode"],"fps":fps,"frame_count":frames.len(),"duration_ms":timing,"layers":layers.iter().enumerate().map(|(i,l)|json!({"index":i+1,"name":l["name"],"visible":l["visible"],"opacity":l["opacity"],"type":l["type"],"blend_mode":l["blend_mode"],"link_sets":l.get("link_sets").cloned().unwrap_or(json!([]))})).collect::<Vec<_>>(),"tags":d["tags"],"unsupported":issues,"notes":["inspect reads metadata; render/unpack validate and read actual cel buffers","supported-subset conversion preserves pixels, timing and links; saved native swatches and author/license fields are retained; editor view/export settings, UI colours and arbitrary extra archive files are not archived"]}),
    )
}

fn annotation(metadata: BTreeMap<String, Value>, user_data: String) -> Annotations {
    Annotations {
        metadata,
        user_data,
    }
}
fn reserved_id(meta: &mut BTreeMap<String, Value>, key: &str, fallback: String) -> Result<String> {
    let id = match meta.remove(key) {
        Some(Value::String(id)) => id,
        Some(_) => anyhow::bail!("invalid reserved metadata {key}"),
        None => fallback,
    };
    schema::identifier(&id)?;
    Ok(id)
}
fn unique_id(base: &str, exists: impl Fn(&str) -> bool) -> String {
    if !exists(base) {
        return base.to_owned();
    }
    let stem = &base[..base.len().min(100)];
    for n in 2.. {
        let candidate = format!("{stem}_{n}");
        if !exists(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}
pub fn read(path: &Path) -> Result<Sprite> {
    let (mut archive, d) = load(path)?;
    let issues = unsupported(&d);
    ensure!(
        issues.is_empty(),
        "unsupported PXO features: {}",
        issues.join("; ")
    );
    let mut h: Header = serde_json::from_value(d).context("invalid PXO metadata")?;
    ensure!(
        h.pxo_version == 7 && h.color_mode == 5,
        "unsupported PXO representation"
    );
    let bytes = schema::pixel_bytes(h.size_x, h.size_y)?;
    ensure!(
        !h.frames.is_empty() && h.frames.len() <= schema::MAX_FRAMES,
        "invalid frame count"
    );
    ensure!(
        !h.layers.is_empty() && h.layers.len() <= schema::MAX_LAYERS,
        "invalid layer count"
    );
    ensure!(
        h.frames.len() * h.layers.len() <= schema::MAX_SLOTS,
        "too many timeline cels"
    );
    schema::positive(h.fps, "fps")?;
    for f in &h.frames {
        schema::positive(f.duration, "PXO duration")?;
        ensure!(
            f.cels.len() == h.layers.len(),
            "frame cel count does not match layers"
        );
    }
    let mut images: BTreeMap<String, Arc<[u8]>> = BTreeMap::new();
    let mut hashes: HashMap<u64, Vec<String>> = HashMap::new();
    let mut resident = 0usize;
    let mut layers = Vec::new();
    let mut layer_ids = HashSet::new();
    for (l, raw) in h.layers.iter_mut().enumerate() {
        let mut membership = vec![None; h.frames.len()];
        for (g, group) in raw.link_sets.iter().enumerate() {
            ensure!(!group.cels.is_empty(), "empty link set on layer {}", l + 1);
            for &f in &group.cels {
                ensure!(f < h.frames.len(), "link frame index {f} out of range");
                ensure!(
                    membership[f].replace(g).is_none(),
                    "duplicate or overlapping link membership"
                );
            }
        }
        let hint = reserved_id(&mut raw.metadata, "_spritec_id", format!("layer_{}", l + 1))?;
        let id = unique_id(&hint, |id| layer_ids.contains(id));
        layer_ids.insert(id.clone());
        let mut timeline = Vec::new();
        let mut group_data: HashMap<usize, (String, Option<String>)> = HashMap::new();
        let mut group_ids = HashSet::new();
        // Link source is the first serialized member, not necessarily the first timeline frame.
        let order: Vec<usize> = raw
            .link_sets
            .iter()
            .map(|g| g.cels[0])
            .chain(0..h.frames.len())
            .collect();
        let mut entries: Vec<Option<Entry>> = vec![None; h.frames.len()];
        for f in order {
            if entries[f].is_some() {
                continue;
            }
            let raw_cel = &mut h.frames[f].cels[l];
            let data = archive.read(
                &format!("image_data/frames/{}/layer_{}", f + 1, l + 1),
                bytes as u64,
            )?;
            ensure!(
                data.len() == bytes,
                "frame {} layer {}: incorrect RGBA buffer length",
                f + 1,
                l + 1
            );
            let explicit_id = raw_cel.metadata.remove("_spritec_cel_id");
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            data.hash(&mut hasher);
            let hash = hasher.finish();
            let mut name = if let Some(v) = explicit_id {
                if v.is_null() && data.iter().all(|b| *b == 0) {
                    None
                } else if v.is_null() {
                    Some(format!("cel_f{}_l{}", f + 1, l + 1))
                } else {
                    Some(v.as_str().context("invalid _spritec_cel_id")?.to_owned())
                }
            } else if let Some(existing) = hashes
                .get(&hash)
                .and_then(|ids| ids.iter().find(|id| *images[*id].as_ref() == *data))
            {
                Some(existing.clone())
            } else {
                Some(format!("cel_f{}_l{}", f + 1, l + 1))
            };
            // Native independent edits may split a definition; provenance is a naming hint.
            if let Some(id) = &name {
                schema::identifier(id)?;
                if images.get(id).is_some_and(|image| image.as_ref() != data) {
                    name = Some(
                        hashes
                            .get(&hash)
                            .and_then(|ids| ids.iter().find(|id| images[*id].as_ref() == data))
                            .cloned()
                            .unwrap_or_else(|| unique_id(id, |key| images.contains_key(key))),
                    );
                }
            }
            if let Some(name) = &name {
                schema::identifier(name)?;
                if let Some(existing) = images.get(name) {
                    ensure!(
                        existing.as_ref() == data,
                        "cel id {name} has conflicting pixels"
                    );
                } else {
                    resident = resident.checked_add(bytes).context("cel memory overflow")?;
                    ensure!(
                        resident <= schema::MAX_IMAGE_BYTES,
                        "unique cel data exceeds 256 MiB memory budget"
                    );
                    images.insert(name.clone(), Arc::from(data));
                    hashes.entry(hash).or_default().push(name.clone());
                }
            } else {
                ensure!(
                    data.iter().all(|b| *b == 0),
                    "null cel contains nonempty pixel bytes"
                );
            }
            let link = if let Some(g) = membership[f] {
                let hint = reserved_id(
                    &mut raw_cel.metadata,
                    "_spritec_link",
                    format!("link_{}", g + 1),
                )?;
                let link = if let Some((old_link, old_cel)) = group_data.get(&g) {
                    let equal = match (old_cel.as_ref(), name.as_ref()) {
                        (Some(a), Some(b)) => images[a] == images[b],
                        (None, None) => true,
                        (Some(a), None) => images[a].iter().all(|b| *b == 0),
                        (None, Some(b)) => images[b].iter().all(|b| *b == 0),
                    };
                    ensure!(
                        equal,
                        "linked cel buffers differ; refusing to discard pixels"
                    );
                    old_link.clone()
                } else {
                    let link = unique_id(&hint, |key| group_ids.contains(key));
                    group_ids.insert(link.clone());
                    group_data.insert(g, (link.clone(), name.clone()));
                    link
                };
                Some(link)
            } else {
                raw_cel.metadata.remove("_spritec_link");
                None
            };
            let cel = membership[f]
                .and_then(|g| group_data.get(&g).map(|p| p.1.clone()))
                .unwrap_or(name);
            entries[f] = Some(Entry {
                cel,
                link,
                opacity: raw_cel.opacity,
                annotations: annotation(
                    std::mem::take(&mut raw_cel.metadata),
                    raw_cel.user_data.clone(),
                ),
            });
        }
        timeline.extend(entries.into_iter().map(Option::unwrap));
        layers.push(Layer {
            id,
            name: raw.name.clone(),
            visible: raw.visible,
            opacity: raw.opacity,
            locked: raw.locked,
            new_cels_linked: raw.new_cels_linked,
            timeline,
            annotations: annotation(std::mem::take(&mut raw.metadata), raw.user_data.clone()),
        });
    }
    let frames = h
        .frames
        .into_iter()
        .map(|f| Frame {
            duration_ms: f.duration / h.fps * 1000.0,
            annotations: annotation(f.metadata, f.user_data),
        })
        .collect();
    let s = Sprite {
        width: h.size_x,
        height: h.size_y,
        fps: h.fps,
        layers,
        frames,
        tags: h.tags,
        annotations: annotation(h.metadata, h.user_data),
        pixelorama: h.pixelorama,
        images,
    };
    schema::validate_model(&s)?;
    Ok(s)
}

fn metadata(a: &Annotations) -> Value {
    json!(a.metadata)
}
fn add<W: Write + Seek>(zip: &mut zip::ZipWriter<W>, name: &str, bytes: &[u8]) -> Result<()> {
    zip.start_file(
        name,
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(6)),
    )?;
    zip.write_all(bytes)?;
    Ok(())
}
pub fn write<W: Write + Seek>(writer: W, s: &Sprite) -> Result<()> {
    schema::validate_model(s)?;
    let mut layers = Vec::new();
    for l in &s.layers {
        let mut groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (f, e) in l.timeline.iter().enumerate() {
            if let Some(link) = &e.link {
                groups.entry(link).or_default().push(f);
            }
        }
        let mut meta = metadata(&l.annotations);
        meta["_spritec_id"] = json!(l.id);
        let mut layer = json!({"name":l.name,"type":0,"visible":l.visible,"locked":l.locked,"blend_mode":0,"clipping_mask":false,"opacity":l.opacity,"parent":-1,"effects":[],"animated_params":"Dictionary[String, Dictionary]({\n\"opacity\": {}\n})","new_cels_linked":l.new_cels_linked,"ui_color":"(0.0, 0.0, 0.0, 0.0)","metadata":meta,"user_data":l.annotations.user_data});
        if !groups.is_empty() {
            layer["link_sets"] = json!(
                groups
                    .values()
                    .map(|frames| json!({"cels":frames,"hue":0.333333343267441}))
                    .collect::<Vec<_>>()
            );
        }
        layers.push(layer);
    }
    let frames:Vec<_>=s.frames.iter().enumerate().map(|(f,frame)| {
        let cels:Vec<_>=s.layers.iter().map(|l|{let e=&l.timeline[f];let mut meta=metadata(&e.annotations);meta["_spritec_cel_id"]=json!(e.cel);if let Some(link)=&e.link{meta["_spritec_link"]=json!(link);}
          json!({"opacity":e.opacity,"z_index":0,"ui_color":"(0.0, 0.0, 0.0, 0.0)","metadata":meta,"user_data":e.annotations.user_data})}).collect();
        json!({"cels":cels,"duration":frame.duration_ms/1000.0*s.fps,"metadata":metadata(&frame.annotations),"user_data":frame.annotations.user_data})
    }).collect();
    let tags:Vec<_>=s.tags.iter().map(|t|json!({"name":t.name,"from":t.from,"to":t.to,"color":t.color[1..].to_ascii_lowercase(),"user_data":t.user_data})).collect();
    let data = json!({"pixelorama_version":"v1.2.3-stable","pxo_version":7,"color_mode":5,"size_x":s.width,"size_y":s.height,"fps":s.fps,"layers":layers,"frames":frames,"tags":tags,"current_frame":0,"current_layer":0,"next_keyframe_id":0,"metadata":metadata(&s.annotations),"user_data":s.annotations.user_data,"brushes":[],"reference_images":[],"tilesets":[],"palettes":s.pixelorama.palettes,"guides":[],"vanishing_points":[],"project_current_palette_name":s.pixelorama.project_current_palette_name,"symmetry_points":[s.width as f64-1.0,s.height as f64-1.0],"tile_mode_x_basis_x":s.width,"tile_mode_x_basis_y":0,"tile_mode_y_basis_x":0,"tile_mode_y_basis_y":s.height,"export_profile":{}});
    let mut data = data;
    for (key, value) in [
        ("license", &s.pixelorama.license),
        ("author_display_name", &s.pixelorama.author_display_name),
        ("author_real_name", &s.pixelorama.author_real_name),
        ("author_contact", &s.pixelorama.author_contact),
        ("author_company", &s.pixelorama.author_company),
    ] {
        data[key] = json!(value);
    }
    let encoded = serde_json::to_vec(&data)?;
    ensure!(
        encoded.len() <= 16 * 1024 * 1024,
        "PXO metadata exceeds 16 MiB"
    );
    let entry_count = 3 + s.frames.len() * s.layers.len() * 2;
    ensure!(
        entry_count <= crate::archive::MAX_ENTRIES,
        "PXO entry count exceeds archive policy"
    );
    let expanded = (s.frames.len() * s.layers.len()) as u64
        * (schema::pixel_bytes(s.width, s.height)? as u64 + 1)
        + encoded.len() as u64;
    ensure!(
        expanded <= crate::archive::MAX_EXPANDED - 64 * 1024 * 1024,
        "PXO exceeds decompressed archive budget"
    );
    let mut zip = zip::ZipWriter::new(writer);
    add(&mut zip, "data.json", &encoded)?;
    add(&mut zip, "mimetype", b"application/x-pixelorama")?;
    let preview = raster::render(s, 0)?;
    let mut png = Vec::new();
    raster::png(&mut png, s.width, s.height, &preview)?;
    add(&mut zip, "preview.png", &png)?;
    let empty = vec![0; schema::pixel_bytes(s.width, s.height)?];
    for f in 0..s.frames.len() {
        for (l, layer) in s.layers.iter().enumerate() {
            let data = layer.timeline[f]
                .cel
                .as_ref()
                .map_or(empty.as_slice(), |id| s.images[id].as_ref());
            add(
                &mut zip,
                &format!("image_data/frames/{}/layer_{}", f + 1, l + 1),
                data,
            )?;
            add(
                &mut zip,
                &format!("image_data/frames/{}/indices_layer_{}", f + 1, l + 1),
                &[0],
            )?;
        }
    }
    zip.finish()?;
    Ok(())
}
