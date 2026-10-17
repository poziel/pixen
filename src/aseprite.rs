//! Bounded RGBA subset of the official Aseprite binary file format.
use crate::{
    model::Sprite,
    schema::{self, Annotations, Entry, Frame, Layer, PixeloramaExtras, Tag},
};
use anyhow::{Context, Result, bail, ensure};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    sync::Arc,
};

const MAX_FILE: u64 = 256 * 1024 * 1024;
const MAX_CHUNK: usize = 32 * 1024 * 1024;
const MAX_METADATA: usize = 16 * 1024 * 1024;
const NATIVE: &str = "aseprite_native";
const ENVELOPE: &str = "spritex-aseprite-v1:";

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    #[serde(default)]
    annotations: Annotations,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pixelorama: Option<PixeloramaExtras>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    frame_annotations: Vec<Annotations>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    empty_entries: BTreeMap<usize, Entry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    singleton: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    empty: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    empty_opacity: Option<f64>,
}

struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}
impl<'a> Bytes<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .context("Aseprite field size overflow")?;
        let out = self
            .data
            .get(self.pos..end)
            .context("truncated Aseprite chunk")?;
        self.pos = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into()?))
    }
    fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn text(&mut self) -> Result<String> {
        let n = self.u16()? as usize;
        Ok(std::str::from_utf8(self.take(n)?)
            .context("Aseprite string is not UTF-8")?
            .to_owned())
    }
    fn end(&self) -> Result<()> {
        ensure!(
            self.pos == self.data.len(),
            "unhandled trailing Aseprite chunk data"
        );
        Ok(())
    }
}

#[derive(Default)]
struct Cel {
    image: Option<Arc<[u8]>>,
    target: Option<usize>,
    x: i16,
    y: i16,
    opacity: u8,
    envelope: Envelope,
}
struct Parsed {
    width: u32,
    height: u32,
    depth: u16,
    frames: Vec<Frame>,
    layers: Vec<Layer>,
    tags: Vec<Tag>,
    cels: Vec<Vec<Option<Cel>>>,
    project: Envelope,
    unsupported: Vec<String>,
    chunks: BTreeMap<String, usize>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Object {
    Project,
    Layer(usize),
    Cel(usize, usize),
    Tags(usize, usize),
    None,
}

fn native_object(a: &mut Annotations) -> Result<&mut serde_json::Map<String, Value>> {
    a.metadata
        .entry(NATIVE.into())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .context("aseprite_native metadata must be an object")
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(text: &str) -> Result<Vec<u8>> {
    ensure!(
        text.len() & 1 == 0 && text.len() <= MAX_METADATA * 2 && text.is_ascii(),
        "invalid Aseprite native hex data"
    );
    (0..text.len())
        .step_by(2)
        .map(|i| Ok(u8::from_str_radix(&text[i..i + 2], 16)?))
        .collect()
}
fn keep_chunk(
    a: &mut Annotations,
    frame: usize,
    kind: u16,
    data: &[u8],
    budget: &mut usize,
) -> Result<()> {
    *budget = budget
        .checked_add(data.len())
        .context("Aseprite metadata size overflow")?;
    ensure!(
        *budget <= MAX_METADATA,
        "Aseprite native metadata exceeds 16 MiB"
    );
    native_object(a)?
        .entry("chunks")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .context("invalid native chunks")?
        .push(json!({"frame":frame,"kind":kind,"hex":hex(data)}));
    Ok(())
}

fn validate_palette(kind: u16, c: &mut Bytes<'_>) -> Result<()> {
    if kind == 0x2019 {
        let size = c.u32()?;
        let first = c.u32()?;
        let last = c.u32()?;
        ensure!(
            size <= 1_048_576 && first <= last && last < size,
            "invalid Aseprite palette bounds"
        );
        c.take(8)?;
        for _ in first..=last {
            let flags = c.u16()?;
            ensure!(flags & !1 == 0, "unsupported Aseprite palette entry flags");
            c.take(4)?;
            if flags & 1 != 0 {
                c.text()?;
            }
        }
    } else {
        let packets = c.u16()?;
        for _ in 0..packets {
            c.u8()?;
            let count = c.u8()?;
            let count = if count == 0 { 256 } else { usize::from(count) };
            let rgb = c.take(count * 3)?;
            ensure!(
                kind != 0x0011 || rgb.iter().all(|v| *v <= 63),
                "invalid 6-bit Aseprite palette color"
            );
        }
    }
    c.end()
}

fn parse(path: &Path, pixels: bool) -> Result<Parsed> {
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let size = file.metadata()?.len();
    ensure!(
        (128..=MAX_FILE).contains(&size),
        "Aseprite file must be 128 bytes..256 MiB"
    );
    let mut header = [0; 128];
    file.read_exact(&mut header)?;
    let mut b = Bytes::new(&header);
    ensure!(
        u64::from(b.u32()?) == size,
        "Aseprite declared file size does not match actual bytes"
    );
    ensure!(b.u16()? == 0xa5e0, "invalid Aseprite file signature");
    let count = b.u16()? as usize;
    ensure!(
        (1..=schema::MAX_FRAMES).contains(&count),
        "Aseprite frame count exceeds supported limits"
    );
    let width = u32::from(b.u16()?);
    let height = u32::from(b.u16()?);
    let image_bytes = schema::pixel_bytes(width, height)?;
    let depth = b.u16()?;
    let flags = b.u32()?;
    let mut p = Parsed {
        width,
        height,
        depth,
        frames: Vec::with_capacity(count),
        layers: Vec::new(),
        tags: Vec::new(),
        cels: Vec::new(),
        project: Envelope::default(),
        unsupported: Vec::new(),
        chunks: BTreeMap::new(),
    };
    if depth != 32 {
        p.unsupported.push(format!(
            "color depth {depth}; only RGBA 32-bit is supported"
        ));
    }
    if flags & !3 != 0 {
        p.unsupported
            .push("unknown header flags or layer UUIDs are unsupported".into());
    }
    if header[34] != 0 && header[35] != 0 && header[34] != header[35] {
        p.unsupported
            .push("non-square pixel ratio is unsupported".into());
    }
    // Keep the documented nonvisual header fields; sizes/timing are always native authoritative.
    native_object(&mut p.project.annotations)?.insert("header".into(), json!(hex(&header[18..44])));
    let mut position = 128u64;
    let mut resident = 0usize;
    let mut metadata = 0usize;
    let mut object = Object::None;
    let mut seen_project = false;
    let mut seen_user_data = HashSet::new();
    for frame in 0..count {
        ensure!(
            position.checked_add(16).is_some_and(|end| end <= size),
            "truncated Aseprite frame header"
        );
        let mut frame_header = [0; 16];
        file.read_exact(&mut frame_header)?;
        let mut f = Bytes::new(&frame_header);
        let frame_size = u64::from(f.u32()?);
        ensure!(
            frame_size >= 16,
            "Aseprite frame size is smaller than its header"
        );
        let end = position
            .checked_add(frame_size)
            .filter(|end| *end <= size)
            .context("Aseprite frame exceeds file bounds")?;
        ensure!(f.u16()? == 0xf1fa, "invalid Aseprite frame signature");
        let old_chunks = u32::from(f.u16()?);
        let duration = f.u16()?;
        ensure!(
            duration > 0,
            "Aseprite frame duration must be 1..65535 milliseconds"
        );
        f.take(2)?;
        let new_chunks = f.u32()?;
        let count_chunks = if new_chunks == 0 {
            old_chunks
        } else {
            new_chunks
        };
        ensure!(
            u64::from(count_chunks) <= (frame_size - 16) / 6,
            "Aseprite chunk count exceeds frame bounds"
        );
        p.frames.push(Frame {
            duration_ms: f64::from(duration),
            annotations: Annotations::default(),
        });
        position += 16;
        for _ in 0..count_chunks {
            ensure!(position + 6 <= end, "truncated Aseprite chunk header");
            let mut chunk_header = [0; 6];
            file.read_exact(&mut chunk_header)?;
            let mut h = Bytes::new(&chunk_header);
            let chunk_size = u64::from(h.u32()?);
            let kind = h.u16()?;
            ensure!(
                chunk_size >= 6 && position.checked_add(chunk_size).is_some_and(|n| n <= end),
                "Aseprite chunk exceeds frame bounds"
            );
            ensure!(
                chunk_size - 6 <= MAX_CHUNK as u64,
                "Aseprite chunk exceeds 32 MiB limit"
            );
            let mut data = vec![0; (chunk_size - 6) as usize];
            file.read_exact(&mut data)?;
            *p.chunks.entry(format!("0x{kind:04X}")).or_default() += 1;
            let mut c = Bytes::new(&data);
            match kind {
                0x2004 => {
                    ensure!(
                        frame == 0,
                        "Aseprite layer definitions outside the first frame are unsupported"
                    );
                    ensure!(
                        p.layers.len() < schema::MAX_LAYERS
                            && (p.layers.len() + 1) * count <= schema::MAX_SLOTS,
                        "Aseprite layer/slot count exceeds supported limits"
                    );
                    let lf = c.u16()?;
                    let ty = c.u16()?;
                    let child = c.u16()?;
                    c.take(4)?;
                    let blend = c.u16()?;
                    let opacity = c.u8()?;
                    c.take(3)?;
                    let name = c.text()?;
                    if ty != 0 || child != 0 {
                        p.unsupported.push(format!(
                            "layer {name:?}: groups, nesting, and tilemaps are unsupported"
                        ));
                    }
                    if blend != 0 {
                        p.unsupported
                            .push(format!("layer {name:?}: blend mode {blend} is unsupported"));
                    }
                    if lf & !(1 | 2 | 16) != 0 {
                        p.unsupported.push(format!("layer {name:?}: background, movement lock, reference, collapsed, or unknown flags are unsupported"));
                    }
                    if ty == 2 {
                        c.u32()?;
                    }
                    if flags & 4 != 0 {
                        c.take(16)?;
                    }
                    c.end()?;
                    let i = p.layers.len();
                    p.layers.push(Layer {
                        id: format!("layer-{i}"),
                        name,
                        visible: lf & 1 != 0,
                        opacity: if flags & 1 == 0 {
                            1.0
                        } else {
                            f64::from(opacity) / 255.0
                        },
                        locked: lf & 2 == 0,
                        new_cels_linked: lf & 16 != 0,
                        timeline: vec![
                            Entry {
                                cel: None,
                                link: None,
                                opacity: 1.0,
                                annotations: Annotations::default()
                            };
                            count
                        ],
                        annotations: Annotations::default(),
                    });
                    p.cels.push((0..count).map(|_| None).collect());
                    object = Object::Layer(i);
                }
                0x2005 => {
                    let layer = c.u16()? as usize;
                    ensure!(
                        layer < p.layers.len(),
                        "Aseprite cel references an undefined layer"
                    );
                    ensure!(
                        p.cels[layer][frame].is_none(),
                        "duplicate Aseprite cel for a layer/frame"
                    );
                    let x = c.i16()?;
                    let y = c.i16()?;
                    let opacity = c.u8()?;
                    let ty = c.u16()?;
                    let z = c.i16()?;
                    c.take(5)?;
                    if z != 0 {
                        p.unsupported
                            .push("cel z-index changes are unsupported".into());
                    }
                    let mut cel = Cel {
                        x,
                        y,
                        opacity,
                        ..Cel::default()
                    };
                    match ty {
                        1 => {
                            let target = c.u16()? as usize;
                            ensure!(target < count, "Aseprite linked cel frame is out of range");
                            cel.target = Some(target);
                            c.end()?;
                        }
                        0 | 2 => {
                            let w = usize::from(c.u16()?);
                            let h = usize::from(c.u16()?);
                            ensure!(
                                w > 0
                                    && h > 0
                                    && x >= 0
                                    && y >= 0
                                    && x as usize + w <= width as usize
                                    && y as usize + h <= height as usize,
                                "Aseprite cropped/offset cel lies outside the canvas"
                            );
                            if pixels && depth == 32 {
                                resident = resident
                                    .checked_add(image_bytes)
                                    .context("Aseprite resident size overflow")?;
                                ensure!(
                                    resident <= schema::MAX_IMAGE_BYTES,
                                    "Aseprite cel buffers exceed memory budget"
                                );
                                let n = w * h * 4;
                                let raw = if ty == 0 {
                                    let raw = c.take(n)?.to_vec();
                                    c.end()?;
                                    raw
                                } else {
                                    let compressed = c.take(data.len() - c.pos)?;
                                    let mut decoder = ZlibDecoder::new(compressed);
                                    let mut raw = Vec::with_capacity(n);
                                    (&mut decoder)
                                        .take(n as u64 + 1)
                                        .read_to_end(&mut raw)
                                        .context("invalid Aseprite zlib cel")?;
                                    ensure!(
                                        raw.len() == n,
                                        "Aseprite decompressed cel byte count mismatch"
                                    );
                                    ensure!(
                                        decoder.total_in() == compressed.len() as u64,
                                        "Aseprite compressed cel has trailing data"
                                    );
                                    raw
                                };
                                let mut rgba = vec![0; image_bytes];
                                for row in 0..h {
                                    let dst =
                                        ((y as usize + row) * width as usize + x as usize) * 4;
                                    rgba[dst..dst + w * 4]
                                        .copy_from_slice(&raw[row * w * 4..(row + 1) * w * 4]);
                                }
                                cel.image = Some(Arc::from(rgba));
                            } else if ty == 0 {
                                let channels = usize::from(depth) / 8;
                                c.take(w * h * channels)?;
                                c.end()?;
                            }
                        }
                        _ => p
                            .unsupported
                            .push(format!("cel type {ty} (including tilemaps) is unsupported")),
                    }
                    p.cels[layer][frame] = Some(cel);
                    object = Object::Cel(layer, frame);
                }
                0x2018 => {
                    ensure!(
                        p.tags.is_empty(),
                        "multiple Aseprite tag chunks are unsupported"
                    );
                    let n = c.u16()? as usize;
                    c.take(8)?;
                    ensure!(n <= schema::MAX_SLOTS, "too many Aseprite tags");
                    for _ in 0..n {
                        let from = c.u16()? as usize + 1;
                        let to = c.u16()? as usize + 1;
                        let direction = c.u8()?;
                        let repeat = c.u16()?;
                        c.take(6)?;
                        let rgb = c.take(3)?;
                        let color = format!("#{:02X}{:02X}{:02X}FF", rgb[0], rgb[1], rgb[2]);
                        c.take(1)?;
                        let name = c.text()?;
                        if direction != 0 || repeat != 0 {
                            p.unsupported.push(format!("tag {name:?}: only forward direction with unspecified repeat count is supported"));
                        }
                        p.tags.push(Tag {
                            name,
                            from,
                            to,
                            color,
                            user_data: String::new(),
                        });
                    }
                    c.end()?;
                    object = Object::Tags(0, n);
                }
                0x2020 => {
                    metadata = metadata
                        .checked_add(data.len())
                        .context("Aseprite metadata size overflow")?;
                    ensure!(
                        metadata <= MAX_METADATA,
                        "Aseprite user/native metadata exceeds 16 MiB"
                    );
                    ensure!(
                        object == Object::None || seen_user_data.insert(object),
                        "duplicate Aseprite user data for one object"
                    );
                    let uf = c.u32()?;
                    if uf & !3 != 0 {
                        p.unsupported.push("Aseprite typed user properties and unknown user-data flags are unsupported".into());
                        object = Object::None;
                        position += chunk_size;
                        continue;
                    }
                    let text = if uf & 1 != 0 {
                        c.text()?
                    } else {
                        String::new()
                    };
                    let color = if uf & 2 != 0 {
                        Some(c.take(4)?.to_vec())
                    } else {
                        None
                    };
                    c.end()?;
                    let mut env = if let Some(text) = text.strip_prefix(ENVELOPE) {
                        serde_json::from_str::<Envelope>(text)
                            .context("invalid Spritex Aseprite user-data envelope")?
                    } else {
                        Envelope {
                            annotations: Annotations {
                                user_data: text,
                                ..Annotations::default()
                            },
                            ..Envelope::default()
                        }
                    };
                    if let Some(color) = color {
                        native_object(&mut env.annotations)?
                            .insert("color".into(), json!(hex(&color)));
                    }
                    match object {
                        Object::Project => {
                            ensure!(
                                env.link.is_none()
                                    && env.id.is_none()
                                    && !env.singleton
                                    && !env.empty
                                    && env.empty_opacity.is_none()
                                    && env.empty_entries.is_empty(),
                                "layer/cel envelope attached to Aseprite project"
                            );
                            ensure!(!seen_project, "duplicate Aseprite project user data");
                            seen_project = true;
                            // Native header/chunks cannot be overridden by a stale metadata envelope.
                            if let Some(existing) = p.project.annotations.metadata.remove(NATIVE) {
                                let target = native_object(&mut env.annotations)?;
                                for (k, v) in
                                    existing.as_object().context("invalid native metadata")?
                                {
                                    target.insert(k.clone(), v.clone());
                                }
                            }
                            p.project = env;
                        }
                        Object::Layer(i) => {
                            ensure!(
                                env.fps.is_none()
                                    && env.pixelorama.is_none()
                                    && env.frame_annotations.is_empty()
                                    && env.link.is_none()
                                    && !env.singleton
                                    && !env.empty
                                    && env.empty_opacity.is_none(),
                                "project/cel envelope attached to Aseprite layer"
                            );
                            if let Some(id) = env.id {
                                schema::identifier(&id)?;
                                p.layers[i].id = id;
                            }
                            p.layers[i].annotations = env.annotations;
                            for (f, e) in env.empty_entries {
                                if f < count {
                                    p.layers[i].timeline[f] = e;
                                } else {
                                    bail!("Aseprite blank-entry metadata frame out of range");
                                }
                            }
                        }
                        Object::Cel(l, f) => {
                            ensure!(
                                env.fps.is_none()
                                    && env.pixelorama.is_none()
                                    && env.frame_annotations.is_empty()
                                    && env.empty_entries.is_empty()
                                    && (env.empty || env.empty_opacity.is_none()),
                                "project/layer envelope attached to Aseprite cel"
                            );
                            p.cels[l][f]
                                .as_mut()
                                .context("user data without cel")?
                                .envelope = env;
                        }
                        Object::Tags(i, n) => {
                            ensure!(i < n, "too many Aseprite tag user-data chunks");
                            ensure!(
                                env.fps.is_none()
                                    && env.pixelorama.is_none()
                                    && env.frame_annotations.is_empty()
                                    && env.empty_entries.is_empty()
                                    && env.link.is_none()
                                    && env.id.is_none()
                                    && !env.singleton
                                    && !env.empty
                                    && env.empty_opacity.is_none()
                                    && env.annotations.metadata.keys().all(|k| k == NATIVE)
                                    && env.annotations.metadata.get(NATIVE).is_none_or(|v| v
                                        .as_object()
                                        .is_some_and(|o| o.keys().all(|k| k == "color"))),
                                "tag metadata is unrepresentable in authoring schema"
                            );
                            p.tags[i].user_data = env.annotations.user_data;
                            if let Some(color) = env
                                .annotations
                                .metadata
                                .get(NATIVE)
                                .and_then(|v| v.get("color"))
                                .and_then(Value::as_str)
                            {
                                let rgba = unhex(color)?;
                                ensure!(rgba.len() == 4, "invalid tag color");
                                p.tags[i].color = format!("#{}", hex(&rgba).to_uppercase());
                            }
                            object = Object::Tags(i + 1, n);
                        }
                        Object::None => p
                            .unsupported
                            .push("Aseprite user data has no representable target".into()),
                    }
                }
                0x0004 | 0x0011 | 0x2019 | 0x2007 => {
                    if kind == 0x2007 {
                        let ty = c.u16()?;
                        let flags = c.u16()?;
                        let gamma = c.u32()?;
                        c.take(8)?;
                        if ty > 1 || flags != 0 || gamma != 0 {
                            p.unsupported.push(
                                "ICC, custom gamma, and unknown color profiles are unsupported"
                                    .into(),
                            );
                        }
                        if ty == 2 {
                            let length = c.u32()? as usize;
                            c.take(length)?;
                        }
                        c.end()?;
                    } else {
                        validate_palette(kind, &mut c)?;
                    }
                    // Preserve palette/profile bytes as editing metadata in RGBA mode.
                    keep_chunk(
                        &mut p.project.annotations,
                        frame,
                        kind,
                        &data,
                        &mut metadata,
                    )?;
                    object = if matches!(kind, 0x0004 | 0x0011 | 0x2019) && frame == 0 {
                        Object::Project
                    } else {
                        Object::None
                    };
                }
                _ => {
                    p.unsupported.push(format!("unhandled chunk 0x{kind:04X} (slices, cel extras, external files, effects, and unknown features are unsupported)"));
                    object = Object::None;
                }
            }
            position += chunk_size;
        }
        ensure!(
            position == end,
            "Aseprite frame contains bytes outside declared chunks"
        );
        object = Object::None;
    }
    ensure!(
        position == size,
        "Aseprite bytes remain after the declared frames"
    );
    Ok(p)
}

/// Read supported native raster data. Unsupported visual/editing features fail explicitly.
pub fn read(path: &Path) -> Result<Sprite> {
    let mut p = parse(path, true)?;
    ensure!(
        p.unsupported.is_empty(),
        "unsupported Aseprite features: {}",
        p.unsupported.join("; ")
    );
    ensure!(!p.layers.is_empty(), "Aseprite has no raster layers");
    if !p.project.frame_annotations.is_empty() {
        ensure!(
            p.project.frame_annotations.len() == p.frames.len(),
            "Aseprite frame-annotation metadata count mismatch"
        );
        for (f, a) in p.frames.iter_mut().zip(p.project.frame_annotations) {
            f.annotations = a;
        }
    }
    let mut images = BTreeMap::new();
    for (layer, slots) in p.cels.iter().enumerate() {
        let mut roots = vec![None; slots.len()];
        for frame in 0..slots.len() {
            if slots[frame].is_none() {
                continue;
            }
            let mut visited = HashSet::new();
            let mut current = frame;
            loop {
                ensure!(visited.insert(current), "Aseprite linked-cel cycle");
                let cel = slots[current]
                    .as_ref()
                    .context("Aseprite linked cel references an empty slot")?;
                if let Some(target) = cel.target {
                    current = target;
                } else {
                    ensure!(cel.image.is_some(), "Aseprite cel has no image data");
                    break;
                }
            }
            roots[frame] = Some(current);
        }
        let mut groups = HashMap::<usize, Vec<usize>>::new();
        for (f, root) in roots.iter().enumerate() {
            if let Some(root) = root {
                groups.entry(*root).or_default().push(f);
            }
        }
        let mut used_names = HashSet::new();
        for (root, members) in groups {
            let source = slots[root]
                .as_ref()
                .context("missing Aseprite source cel")?;
            let image = source.image.clone().context("missing Aseprite image")?;
            let empty = image.iter().all(|b| *b == 0)
                && members
                    .iter()
                    .all(|f| slots[*f].as_ref().is_some_and(|c| c.envelope.empty));
            let mut id = source
                .envelope
                .id
                .clone()
                .unwrap_or_else(|| format!("cel-{layer}-{root}"));
            schema::identifier(&id)?;
            if images
                .get(&id)
                .is_some_and(|existing: &Arc<[u8]>| existing.as_ref() != image.as_ref())
            {
                id = format!("cel-{layer}-{root}");
                while images.contains_key(&id) {
                    id.push('x');
                }
            }
            if !empty {
                images.insert(id.clone(), image);
            }
            let actual_link = members.len() > 1;
            let hint = source.envelope.link.as_deref();
            let name = if actual_link {
                if hint.is_some()
                    && members.iter().all(|f| {
                        slots[*f]
                            .as_ref()
                            .is_some_and(|c| c.envelope.link.as_deref() == hint)
                    })
                {
                    hint.map(str::to_owned)
                } else {
                    Some(format!("link-{layer}-{root}"))
                }
            } else if source.envelope.singleton {
                source.envelope.link.clone()
            } else {
                None
            };
            if let Some(name) = &name {
                schema::identifier(name)?;
                ensure!(
                    used_names.insert(name.clone()),
                    "Aseprite metadata assigns one link name to separate native groups"
                );
            }
            for frame in members {
                let cel = slots[frame].as_ref().context("missing Aseprite cel")?;
                ensure!(
                    cel.x == source.x && cel.y == source.y,
                    "linked Aseprite cels with different offsets cannot preserve editing links in the authoring model"
                );
                p.layers[layer].timeline[frame] = Entry {
                    cel: if empty { None } else { Some(id.clone()) },
                    link: name.clone(),
                    opacity: cel
                        .envelope
                        .empty_opacity
                        .filter(|hint| empty && opacity(*hint) == cel.opacity)
                        .unwrap_or_else(|| f64::from(cel.opacity) / 255.0),
                    annotations: cel.envelope.annotations.clone(),
                };
            }
        }
        for (f, slot) in slots.iter().enumerate() {
            if slot.is_none() {
                ensure!(
                    p.layers[layer].timeline[f].cel.is_none(),
                    "blank-entry metadata may not invent Aseprite pixels"
                );
            }
        }
    }
    let fps = p
        .project
        .fps
        .unwrap_or_else(|| 1000.0 / p.frames[0].duration_ms);
    let s = Sprite {
        width: p.width,
        height: p.height,
        fps,
        annotations: p.project.annotations,
        pixelorama: p.project.pixelorama.unwrap_or_default(),
        frames: p.frames,
        layers: p.layers,
        tags: p.tags,
        images,
    };
    schema::validate_model(&s)?;
    Ok(s)
}

/// Inspect structure and metadata without decompressing or validating raster payloads.
pub fn inspect(path: &Path) -> Result<Value> {
    let p = parse(path, false)?;
    Ok(
        json!({"format":"aseprite","width":p.width,"height":p.height,"color_depth":p.depth,"frame_count":p.frames.len(),"layer_count":p.layers.len(),"frames":p.frames,"layers":p.layers.iter().map(|l|json!({"name":l.name,"visible":l.visible,"locked":l.locked,"opacity":l.opacity,"new_cels_linked":l.new_cels_linked})).collect::<Vec<_>>(),"tags":p.tags,"chunks":p.chunks,"unsupported_features":p.unsupported,"raster_validated":false,"inspection":"metadata only; compressed raster payloads and linked-cel graphs are not validated"}),
    )
}

fn put16(out: &mut Vec<u8>, n: u16) {
    out.extend_from_slice(&n.to_le_bytes());
}
fn put32(out: &mut Vec<u8>, n: u32) {
    out.extend_from_slice(&n.to_le_bytes());
}
fn string(out: &mut Vec<u8>, s: &str) -> Result<()> {
    put16(
        out,
        u16::try_from(s.len()).context("Aseprite text exceeds 65535 UTF-8 bytes")?,
    );
    out.extend_from_slice(s.as_bytes());
    Ok(())
}
fn opacity(n: f64) -> u8 {
    (n * 255.0).round() as u8
}
fn chunk<W: Write>(writer: &mut W, kind: u16, data: &[u8], metadata: &mut usize) -> Result<()> {
    ensure!(
        data.len() <= MAX_CHUNK,
        "Aseprite output chunk exceeds 32 MiB"
    );
    if matches!(kind, 0x0004 | 0x0011 | 0x2019 | 0x2007 | 0x2020) {
        *metadata = metadata
            .checked_add(data.len())
            .context("Aseprite output metadata size overflow")?;
        ensure!(
            *metadata <= MAX_METADATA,
            "Aseprite output native/user metadata exceeds 16 MiB"
        );
    }
    writer.write_all(&u32::try_from(data.len() + 6)?.to_le_bytes())?;
    writer.write_all(&kind.to_le_bytes())?;
    writer.write_all(data)?;
    Ok(())
}
fn user_data<W: Write>(
    writer: &mut W,
    env: &Envelope,
    color: Option<[u8; 4]>,
    metadata: &mut usize,
) -> Result<()> {
    let color = if color.is_some() {
        color
    } else if let Some(v) = env
        .annotations
        .metadata
        .get(NATIVE)
        .and_then(|v| v.get("color"))
    {
        Some(
            unhex(v.as_str().context("invalid native user color")?)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("native Aseprite user color must have four bytes"))?,
        )
    } else {
        None
    };
    struct Text {
        bytes: Vec<u8>,
    }
    impl Write for Text {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self
                .bytes
                .len()
                .checked_add(bytes.len())
                .is_none_or(|n| n > u16::MAX as usize)
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Aseprite text exceeds 65535 UTF-8 bytes",
                ));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut encoded = Text {
        bytes: ENVELOPE.as_bytes().to_vec(),
    };
    serde_json::to_writer(&mut encoded, env)
        .context("Aseprite user data exceeds 65535 UTF-8 bytes or is not serializable")?;
    let mut data = Vec::new();
    put32(&mut data, if color.is_some() { 3 } else { 1 });
    put16(&mut data, encoded.bytes.len() as u16);
    data.extend_from_slice(&encoded.bytes);
    if let Some(color) = color {
        data.extend_from_slice(&color);
    }
    chunk(writer, 0x2020, &data, metadata)
}

// Count the buffers the native reader will retain, rather than authoring image IDs.
// Each independent occurrence is a separate native raster source even when pixels agree.
fn native_raster_bytes(s: &Sprite) -> Result<usize> {
    let image_bytes = schema::pixel_bytes(s.width, s.height)?;
    let mut sources = 0usize;
    for layer in &s.layers {
        let mut links = HashSet::new();
        for entry in &layer.timeline {
            if let Some(link) = entry.link.as_deref() {
                if !links.insert(link) {
                    continue;
                }
            } else if entry.cel.is_none() {
                continue;
            }
            sources = sources
                .checked_add(1)
                .context("Aseprite raster source count overflow")?;
        }
    }
    let resident = sources
        .checked_mul(image_bytes)
        .context("Aseprite resident size overflow")?;
    ensure!(
        resident <= schema::MAX_IMAGE_BYTES,
        "Aseprite native raster sources require {resident} bytes, exceeding the {} byte resident buffer limit; each independent cel occurrence counts separately and each explicit link group counts once",
        schema::MAX_IMAGE_BYTES
    );
    Ok(resident)
}

/// Write full-canvas compressed RGBA cels. Only explicit authoring links become native links.
pub fn write<W: Write + Seek>(mut writer: W, s: &Sprite) -> Result<()> {
    schema::validate_model(s)?;
    native_raster_bytes(s)?;
    let mut output_metadata = 0;
    let referenced: HashSet<_> = s
        .layers
        .iter()
        .flat_map(|l| l.timeline.iter())
        .filter_map(|e| e.cel.as_deref())
        .collect();
    ensure!(
        s.images.keys().all(|id| referenced.contains(id.as_str())),
        "Aseprite cannot preserve unused cel definitions; remove unused definitions or reference them in the timeline"
    );
    for f in &s.frames {
        ensure!(
            f.duration_ms.fract() == 0.0 && f.duration_ms <= 65535.0,
            "Aseprite requires integer frame durations in 1..65535 milliseconds; fractional timing is not rounded"
        );
    }
    ensure!(s.tags.len() <= u16::MAX as usize, "too many Aseprite tags");
    let mut header = [0; 128];
    if let Some(v) = s
        .annotations
        .metadata
        .get(NATIVE)
        .and_then(|v| v.get("header"))
    {
        let bytes = unhex(v.as_str().context("invalid native Aseprite header")?)?;
        ensure!(bytes.len() == 26, "invalid native Aseprite header length");
        header[18..44].copy_from_slice(&bytes);
    }
    header[4..6].copy_from_slice(&0xa5e0u16.to_le_bytes());
    header[6..8].copy_from_slice(&(s.frames.len() as u16).to_le_bytes());
    header[8..10].copy_from_slice(&(s.width as u16).to_le_bytes());
    header[10..12].copy_from_slice(&(s.height as u16).to_le_bytes());
    header[12..14].copy_from_slice(&32u16.to_le_bytes());
    header[14..18].copy_from_slice(&1u32.to_le_bytes());
    header[18..20].copy_from_slice(&(s.frames[0].duration_ms as u16).to_le_bytes());
    header[34] = 1;
    header[35] = 1;
    let start = writer.stream_position()?;
    writer.write_all(&header)?;
    let mut native_chunks = BTreeMap::<usize, Vec<(u16, Vec<u8>)>>::new();
    if let Some(chunks) = s
        .annotations
        .metadata
        .get(NATIVE)
        .and_then(|v| v.get("chunks"))
    {
        let chunks = chunks
            .as_array()
            .context("invalid native Aseprite chunks")?;
        let mut total = 0usize;
        for ch in chunks {
            ensure!(
                ch.as_object().is_some_and(|o| o.len() == 3
                    && o.keys()
                        .all(|k| matches!(k.as_str(), "frame" | "kind" | "hex"))),
                "unknown preserved Aseprite native chunk fields"
            );
            let frame = ch["frame"]
                .as_u64()
                .and_then(|v| usize::try_from(v).ok())
                .context("invalid native chunk frame")?;
            let kind = ch["kind"]
                .as_u64()
                .and_then(|v| u16::try_from(v).ok())
                .context("invalid native chunk type")?;
            ensure!(
                frame < s.frames.len() && matches!(kind, 0x0004 | 0x0011 | 0x2019 | 0x2007),
                "unsupported preserved Aseprite native chunk"
            );
            let data = unhex(ch["hex"].as_str().context("invalid native chunk hex")?)?;
            let mut fields = Bytes::new(&data);
            if kind == 0x2007 {
                let ty = fields.u16()?;
                let flags = fields.u16()?;
                let gamma = fields.u32()?;
                fields.take(8)?;
                ensure!(
                    ty <= 1 && flags == 0 && gamma == 0,
                    "unsupported preserved Aseprite color profile"
                );
                fields.end()?;
            } else {
                validate_palette(kind, &mut fields)?;
            }
            total = total
                .checked_add(data.len())
                .context("native chunk size overflow")?;
            ensure!(total <= MAX_METADATA, "native metadata exceeds 16 MiB");
            native_chunks.entry(frame).or_default().push((kind, data));
        }
    }
    let mut link_first: Vec<HashMap<&str, usize>> = vec![HashMap::new(); s.layers.len()];
    let mut link_counts: Vec<HashMap<&str, usize>> = vec![HashMap::new(); s.layers.len()];
    let mut blank: Option<Vec<u8>> = None;
    for (li, l) in s.layers.iter().enumerate() {
        for e in &l.timeline {
            if let Some(link) = e.link.as_deref() {
                *link_counts[li].entry(link).or_default() += 1;
            }
        }
    }
    for (fi, frame) in s.frames.iter().enumerate() {
        let frame_start = writer.stream_position()?;
        writer.write_all(&[0; 16])?;
        let mut chunks = 0u32;
        if let Some(raw) = native_chunks.remove(&fi) {
            for (kind, data) in raw {
                chunk(&mut writer, kind, &data, &mut output_metadata)?;
                chunks += 1;
            }
        }
        if fi == 0 {
            // A palette immediately preceding project user data gives it the specified native target.
            let mut palette = Vec::new();
            put32(&mut palette, 1);
            put32(&mut palette, 0);
            put32(&mut palette, 0);
            palette.extend_from_slice(&[0; 8]);
            put16(&mut palette, 0);
            palette.extend_from_slice(&[0; 4]);
            // Do not replace an independently saved editing palette with the sentinel.
            let has_palette = s
                .annotations
                .metadata
                .get(NATIVE)
                .and_then(|v| v.get("chunks"))
                .and_then(Value::as_array)
                .is_some_and(|cs| {
                    cs.iter().any(|c| {
                        c["frame"] == 0
                            && matches!(c["kind"].as_u64(), Some(0x0004 | 0x0011 | 0x2019))
                    })
                });
            if !has_palette {
                chunk(&mut writer, 0x2019, &palette, &mut output_metadata)?;
                chunks += 1;
            }
            // If a color profile followed the native palette, repeat that palette before user data.
            else if let Some(cs) = s
                .annotations
                .metadata
                .get(NATIVE)
                .and_then(|v| v.get("chunks"))
                .and_then(Value::as_array)
            {
                let last = cs.iter().rfind(|c| c["frame"] == 0);
                if last.is_some_and(|c| c["kind"] == 0x2007) {
                    let pal = cs
                        .iter()
                        .rev()
                        .find(|c| {
                            c["frame"] == 0
                                && matches!(c["kind"].as_u64(), Some(0x0004 | 0x0011 | 0x2019))
                        })
                        .context("missing palette")?;
                    chunk(
                        &mut writer,
                        pal["kind"].as_u64().context("invalid palette")? as u16,
                        &unhex(pal["hex"].as_str().context("invalid palette hex")?)?,
                        &mut output_metadata,
                    )?;
                    chunks += 1;
                }
            }
            let mut annotations = s.annotations.clone();
            // Native bytes are carried as native chunks rather than duplicating them in a 64 KiB string.
            if let Some(native) = annotations
                .metadata
                .get_mut(NATIVE)
                .and_then(Value::as_object_mut)
            {
                native.remove("chunks");
                native.remove("header");
                if native.is_empty() {
                    annotations.metadata.remove(NATIVE);
                }
            }
            user_data(
                &mut writer,
                &Envelope {
                    annotations,
                    fps: Some(s.fps),
                    pixelorama: Some(s.pixelorama.clone()),
                    frame_annotations: s.frames.iter().map(|f| f.annotations.clone()).collect(),
                    ..Envelope::default()
                },
                None,
                &mut output_metadata,
            )?;
            chunks += 1;
            for l in &s.layers {
                let mut data = Vec::new();
                let flags = u16::from(l.visible)
                    | if l.locked { 0 } else { 2 }
                    | if l.new_cels_linked { 16 } else { 0 };
                put16(&mut data, flags);
                data.extend_from_slice(&[0; 10]);
                data.push(opacity(l.opacity));
                data.extend_from_slice(&[0; 3]);
                string(&mut data, &l.name)?;
                chunk(&mut writer, 0x2004, &data, &mut output_metadata)?;
                chunks += 1;
                let empty_entries = l
                    .timeline
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.cel.is_none() && e.link.is_none())
                    .map(|(i, e)| (i, e.clone()))
                    .collect();
                user_data(
                    &mut writer,
                    &Envelope {
                        annotations: l.annotations.clone(),
                        id: Some(l.id.clone()),
                        empty_entries,
                        ..Envelope::default()
                    },
                    None,
                    &mut output_metadata,
                )?;
                chunks += 1;
            }
            if !s.tags.is_empty() {
                let mut data = Vec::new();
                put16(&mut data, s.tags.len() as u16);
                data.extend_from_slice(&[0; 8]);
                for t in &s.tags {
                    put16(&mut data, (t.from - 1) as u16);
                    put16(&mut data, (t.to - 1) as u16);
                    data.extend_from_slice(&[0; 9]);
                    data.extend_from_slice(&schema::decode_color(&t.color)?[..3]);
                    data.push(0);
                    string(&mut data, &t.name)?;
                }
                chunk(&mut writer, 0x2018, &data, &mut output_metadata)?;
                chunks += 1;
                for t in &s.tags {
                    user_data(
                        &mut writer,
                        &Envelope {
                            annotations: Annotations {
                                user_data: t.user_data.clone(),
                                ..Annotations::default()
                            },
                            ..Envelope::default()
                        },
                        Some(schema::decode_color(&t.color)?),
                        &mut output_metadata,
                    )?;
                    chunks += 1;
                }
            }
        }
        for (li, l) in s.layers.iter().enumerate() {
            let e = &l.timeline[fi];
            if e.cel.is_none() && e.link.is_none() {
                continue;
            }
            let previous = e
                .link
                .as_deref()
                .and_then(|link| link_first[li].get(link))
                .copied();
            let mut data = Vec::new();
            put16(&mut data, li as u16);
            data.extend_from_slice(&[0; 4]);
            data.push(opacity(e.opacity));
            put16(&mut data, if previous.is_some() { 1 } else { 2 });
            data.extend_from_slice(&[0; 7]);
            if let Some(previous) = previous {
                put16(&mut data, previous as u16);
            } else {
                put16(&mut data, s.width as u16);
                put16(&mut data, s.height as u16);
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
                if let Some(id) = &e.cel {
                    encoder.write_all(&s.images[id])?;
                } else {
                    if blank.is_none() {
                        blank = Some(vec![0; schema::pixel_bytes(s.width, s.height)?]);
                    }
                    encoder.write_all(blank.as_deref().context("missing blank cel")?)?;
                }
                data.extend_from_slice(&encoder.finish()?);
                if let Some(link) = e.link.as_deref() {
                    link_first[li].insert(link, fi);
                }
            }
            chunk(&mut writer, 0x2005, &data, &mut output_metadata)?;
            chunks += 1;
            user_data(
                &mut writer,
                &Envelope {
                    annotations: e.annotations.clone(),
                    id: e.cel.clone(),
                    link: e.link.clone(),
                    singleton: e
                        .link
                        .as_deref()
                        .is_some_and(|name| link_counts[li][name] == 1),
                    empty: e.cel.is_none(),
                    empty_opacity: if e.cel.is_none() {
                        Some(e.opacity)
                    } else {
                        None
                    },
                    ..Envelope::default()
                },
                None,
                &mut output_metadata,
            )?;
            chunks += 1;
        }
        let end = writer.stream_position()?;
        ensure!(end - start <= MAX_FILE, "Aseprite output exceeds 256 MiB");
        writer.seek(SeekFrom::Start(frame_start))?;
        writer.write_all(&u32::try_from(end - frame_start)?.to_le_bytes())?;
        writer.write_all(&0xf1fau16.to_le_bytes())?;
        writer.write_all(
            &(if chunks >= 65535 {
                65535
            } else {
                chunks as u16
            })
            .to_le_bytes(),
        )?;
        writer.write_all(&(frame.duration_ms as u16).to_le_bytes())?;
        writer.write_all(&[0; 2])?;
        writer.write_all(&chunks.to_le_bytes())?;
        writer.seek(SeekFrom::Start(end))?;
    }
    let end = writer.stream_position()?;
    writer.seek(SeekFrom::Start(start))?;
    writer.write_all(&u32::try_from(end - start)?.to_le_bytes())?;
    writer.seek(SeekFrom::Start(end))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget_sprite(frames: usize) -> Sprite {
        Sprite {
            width: 1024,
            height: 1024,
            fps: 10.0,
            annotations: Annotations::default(),
            pixelorama: PixeloramaExtras::default(),
            frames: vec![
                Frame {
                    duration_ms: 100.0,
                    annotations: Annotations::default()
                };
                frames
            ],
            layers: vec![Layer {
                id: "art".into(),
                name: "Art".into(),
                visible: true,
                opacity: 1.0,
                locked: false,
                new_cels_linked: false,
                annotations: Annotations::default(),
                timeline: vec![
                    Entry {
                        cel: Some("one-definition".into()),
                        link: None,
                        opacity: 1.0,
                        annotations: Annotations::default()
                    };
                    frames
                ],
            }],
            // Source-count arithmetic needs no raster allocation or encoded animation.
            images: BTreeMap::new(),
            tags: Vec::new(),
        }
    }

    #[test]
    fn native_raster_budget_accepts_exact_limit_and_rejects_next_independent_source() {
        assert_eq!(
            native_raster_bytes(&budget_sprite(64)).unwrap(),
            schema::MAX_IMAGE_BYTES
        );
        assert!(
            native_raster_bytes(&budget_sprite(65))
                .unwrap_err()
                .to_string()
                .contains("resident buffer limit")
        );
    }

    #[test]
    fn raster_budget_counts_links_per_layer_and_includes_transparent_link_sources() {
        let mut sprite = budget_sprite(65);
        for entry in &mut sprite.layers[0].timeline {
            entry.link = Some("shared".into());
        }
        let one = schema::pixel_bytes(sprite.width, sprite.height).unwrap();
        assert_eq!(native_raster_bytes(&sprite).unwrap(), one);
        sprite.layers[0].timeline[0].link = None; // An identical, independent native source.
        sprite.layers[0].timeline[1].cel = None;
        sprite.layers[0].timeline[1].link = Some("blank".into());
        sprite.layers[0].timeline[2] = sprite.layers[0].timeline[1].clone();
        sprite.layers[0].timeline[3].cel = None;
        sprite.layers[0].timeline[3].link = None; // An absent, unlinked slot uses no buffer.
        assert_eq!(native_raster_bytes(&sprite).unwrap(), one * 3);
        sprite.layers.push(sprite.layers[0].clone());
        assert_eq!(native_raster_bytes(&sprite).unwrap(), one * 6);
    }
}
