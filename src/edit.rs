//! Typed candidate-document edits; Session owns validation, persistence and history.
use crate::schema::{self, CelDefinition, Document, LegacyCel};
use anyhow::{Context, Result, ensure};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pixel {
    pub x: i64,
    pub y: i64,
    pub palette_index: u32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Drawing {
    SetPixels {
        pixels: Vec<Pixel>,
    },
    Line {
        x1: i64,
        y1: i64,
        x2: i64,
        y2: i64,
        palette_index: u32,
    },
    Rectangle {
        x: i64,
        y: i64,
        width: u32,
        height: u32,
        palette_index: u32,
        filled: bool,
    },
    Fill {
        x: i64,
        y: i64,
        palette_index: u32,
    },
    Clear {
        palette_index: u32,
    },
    Ellipse {
        x: i64,
        y: i64,
        width: u32,
        height: u32,
        palette_index: u32,
        filled: bool,
    },
    EraseRegion {
        x: i64,
        y: i64,
        width: u32,
        height: u32,
    },
    ClearCel {},
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct DrawOperation {
    pub frame_id: String,
    pub layer_id: String,
    #[serde(flatten)]
    pub drawing: Drawing,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LayerOperation {
    AddLayer {
        layer_id: String,
        name: String,
        index: Option<usize>,
    },
    RemoveLayer {
        layer_id: String,
    },
    RenameLayer {
        layer_id: String,
        name: String,
    },
    ReorderLayer {
        layer_id: String,
        index: usize,
    },
    UpdateLayer {
        layer_id: String,
        visible: Option<bool>,
        opacity: Option<f64>,
    },
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FrameOperation {
    AddFrame {
        frame_id: String,
        duration_ms: f64,
        index: Option<usize>,
    },
    RemoveFrame {
        frame_id: String,
    },
    DuplicateFrame {
        frame_id: String,
        new_frame_id: String,
        index: Option<usize>,
    },
    ReorderFrame {
        frame_id: String,
        index: usize,
    },
    SetFrameDuration {
        frame_id: String,
        duration_ms: f64,
    },
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Operation {
    Draw(DrawOperation),
    Layer(LayerOperation),
    Frame(FrameOperation),
    Palette(crate::edit_extra::PaletteOperation),
    Region(crate::edit_extra::RegionOperation),
}

pub const FRAME_ID_KEY: &str = "pixloom_frame_id";
pub fn frame_id(frame: &schema::Frame) -> Option<&str> {
    frame
        .annotations
        .metadata
        .get(FRAME_ID_KEY)
        .and_then(Value::as_str)
}
pub fn frame_index(doc: &Document, id: &str) -> Result<usize> {
    doc.frames
        .iter()
        .position(|f| frame_id(f) == Some(id))
        .context("unknown frame_id; inspect get_project_info for stable IDs")
}
fn layer_index(doc: &Document, id: &str) -> Result<usize> {
    doc.layers
        .iter()
        .position(|l| l.id == id)
        .context("unknown layer_id")
}
pub fn transparent(doc: &mut Document) -> u32 {
    if let Some(i) = doc
        .palette
        .iter()
        .position(|c| schema::decode_color(c).is_ok_and(|c| c[3] == 0))
    {
        return i as u32;
    }
    doc.palette.push("#00000000".into());
    (doc.palette.len() - 1) as u32
}
pub fn grid(doc: &Document, layer: usize, frame: usize) -> Result<Vec<Vec<u32>>> {
    let Some(id) = &doc.layers[layer].timeline[frame].cel else {
        let index = doc
            .palette
            .iter()
            .position(|c| schema::decode_color(c).is_ok_and(|c| c[3] == 0))
            .context("empty cel needs a transparent palette entry")? as u32;
        return Ok(vec![
            vec![index; doc.canvas.width as usize];
            doc.canvas.height as usize
        ]);
    };
    match doc
        .cels
        .iter()
        .find(|c| c.id() == id)
        .context("unknown cel")?
    {
        CelDefinition::Legacy(c) => Ok(c.pixels.clone()),
        CelDefinition::Encoded(c) => c.decode_validated(&doc.canvas, doc.palette.len()),
    }
}
fn edited_cel(version: u32, id: String, pixels: Vec<Vec<u32>>) -> CelDefinition {
    if version == 1 {
        CelDefinition::Legacy(LegacyCel { id, pixels })
    } else {
        CelDefinition::Encoded(crate::encoding::Encoding::Grid(
            crate::encoding::grid::Grid {
                id,
                pixels,
                offset: None,
                size: None,
                default_index: None,
            },
        ))
    }
}
fn new_cel(doc: &mut Document, pixels: Vec<Vec<u32>>, next: &mut u64) -> String {
    loop {
        *next += 1;
        let id = format!("cel-{next}");
        if !doc.cels.iter().any(|c| c.id() == id) {
            doc.cels.push(edited_cel(doc.version, id.clone(), pixels));
            return id;
        }
    }
}
fn blank_entry() -> schema::Entry {
    schema::Entry {
        cel: None,
        link: None,
        opacity: 1.0,
        annotations: Default::default(),
    }
}
fn reserve(used: &mut BTreeSet<String>, kind: &str, id: &str) -> Result<()> {
    schema::identifier(id)?;
    ensure!(
        used.insert(format!("{kind}:{id}")),
        "{kind} ID {id} was already used in this project; choose a new stable ID"
    );
    Ok(())
}
fn insert_frame(doc: &mut Document, id: &str, duration: f64, index: usize) -> Result<()> {
    ensure!(
        index <= doc.frames.len(),
        "frame insertion index out of bounds"
    );
    schema::positive(duration, "duration_ms")?;
    let mut frame = schema::Frame {
        duration_ms: duration,
        annotations: Default::default(),
    };
    frame
        .annotations
        .metadata
        .insert(FRAME_ID_KEY.into(), json!(id));
    doc.frames.insert(index, frame);
    Ok(())
}
fn remap_tags(doc: &mut Document, before: &[String]) -> Result<()> {
    let after: Vec<_> = doc
        .frames
        .iter()
        .map(|f| {
            frame_id(f)
                .map(str::to_owned)
                .context("frame is missing its stable ID")
        })
        .collect::<Result<_>>()?;
    let mut kept = Vec::new();
    for mut tag in std::mem::take(&mut doc.tags) {
        let members = &before[tag.from - 1..tag.to];
        let indices: Vec<_> = after
            .iter()
            .enumerate()
            .filter_map(|(i, id)| members.contains(id).then_some(i))
            .collect();
        if let (Some(&first), Some(&last)) = (indices.first(), indices.last()) {
            ensure!(
                last - first + 1 == indices.len(),
                "operation would make tag {} non-contiguous; keep its existing members together",
                tag.name
            );
            tag.from = first + 1;
            tag.to = last + 1;
            kept.push(tag);
        }
    }
    doc.tags = kept;
    Ok(())
}

pub fn execute(
    doc: &mut Document,
    operation: Operation,
    used: &mut BTreeSet<String>,
    next_cel: &mut u64,
) -> Result<()> {
    match operation {
        Operation::Palette(op) => crate::edit_extra::palette(doc, op, used, next_cel)?,
        Operation::Region(op) => crate::edit_extra::region(doc, op, used, next_cel)?,
        Operation::Draw(op) => {
            let layer = layer_index(doc, &op.layer_id)?;
            let frame = frame_index(doc, &op.frame_id)?;
            ensure!(!doc.layers[layer].locked, "layer is locked");
            let drawing = match op.drawing {
                Drawing::ClearCel {} => Drawing::Clear {
                    palette_index: transparent(doc),
                },
                Drawing::EraseRegion {
                    x,
                    y,
                    width,
                    height,
                } => Drawing::Rectangle {
                    x,
                    y,
                    width,
                    height,
                    palette_index: transparent(doc),
                    filled: true,
                },
                other => other,
            };
            let mut pixels = grid(doc, layer, frame)?;
            draw(&mut pixels, doc.palette.len(), drawing)?;
            let entry = doc.layers[layer].timeline[frame].clone();
            let targets: Vec<_> = doc.layers[layer]
                .timeline
                .iter()
                .enumerate()
                .filter_map(|(i, e)| {
                    (i == frame || entry.link.is_some() && e.link == entry.link).then_some(i)
                })
                .collect();
            let references = entry
                .cel
                .as_ref()
                .map(|id| {
                    doc.layers
                        .iter()
                        .flat_map(|l| &l.timeline)
                        .filter(|e| e.cel.as_ref() == Some(id))
                        .count()
                })
                .unwrap_or(0);
            let id = if references == targets.len()
                && let Some(id) = entry.cel
            {
                let c = doc
                    .cels
                    .iter_mut()
                    .find(|c| c.id() == id)
                    .context("unknown cel")?;
                *c = edited_cel(doc.version, id.clone(), pixels);
                id
            } else {
                new_cel(doc, pixels, next_cel)
            };
            for f in targets {
                doc.layers[layer].timeline[f].cel = Some(id.clone());
            }
        }
        Operation::Layer(op) => match op {
            LayerOperation::AddLayer {
                layer_id,
                name,
                index,
            } => {
                reserve(used, "layer", &layer_id)?;
                ensure!(!name.trim().is_empty(), "layer name must not be empty");
                ensure!(
                    doc.layers.len() < schema::MAX_LAYERS
                        && (doc.layers.len() + 1) * doc.frames.len() <= schema::MAX_SLOTS,
                    "layer/slot limit reached"
                );
                let at = index.unwrap_or(doc.layers.len());
                ensure!(
                    at <= doc.layers.len(),
                    "layer insertion index out of bounds"
                );
                doc.layers.insert(
                    at,
                    schema::Layer {
                        id: layer_id,
                        name,
                        visible: true,
                        opacity: 1.0,
                        locked: false,
                        new_cels_linked: false,
                        timeline: vec![blank_entry(); doc.frames.len()],
                        annotations: Default::default(),
                    },
                );
            }
            LayerOperation::RemoveLayer { layer_id } => {
                let i = layer_index(doc, &layer_id)?;
                ensure!(doc.layers.len() > 1, "cannot remove the last layer");
                doc.layers.remove(i);
            }
            LayerOperation::RenameLayer { layer_id, name } => {
                let i = layer_index(doc, &layer_id)?;
                ensure!(!name.trim().is_empty(), "layer name must not be empty");
                doc.layers[i].name = name;
            }
            LayerOperation::ReorderLayer { layer_id, index } => {
                let i = layer_index(doc, &layer_id)?;
                ensure!(index < doc.layers.len(), "layer index out of bounds");
                let layer = doc.layers.remove(i);
                doc.layers.insert(index, layer);
            }
            LayerOperation::UpdateLayer {
                layer_id,
                visible,
                opacity,
            } => {
                let i = layer_index(doc, &layer_id)?;
                ensure!(
                    visible.is_some() || opacity.is_some(),
                    "provide visibility and/or opacity"
                );
                if let Some(o) = opacity {
                    schema::opacity(o)?;
                    doc.layers[i].opacity = o;
                }
                if let Some(v) = visible {
                    doc.layers[i].visible = v;
                }
            }
        },
        Operation::Frame(op) => {
            let before = doc
                .frames
                .iter()
                .map(|f| {
                    frame_id(f)
                        .map(str::to_owned)
                        .context("frame is missing its stable ID")
                })
                .collect::<Result<Vec<_>>>()?;
            match op {
                FrameOperation::AddFrame {
                    frame_id,
                    duration_ms,
                    index,
                } => {
                    reserve(used, "frame", &frame_id)?;
                    ensure!(
                        doc.frames.len() < schema::MAX_FRAMES
                            && (doc.frames.len() + 1) * doc.layers.len() <= schema::MAX_SLOTS,
                        "frame/slot limit reached"
                    );
                    let at = index.unwrap_or(doc.frames.len());
                    insert_frame(doc, &frame_id, duration_ms, at)?;
                    for layer in &mut doc.layers {
                        layer.timeline.insert(at, blank_entry());
                    }
                }
                FrameOperation::DuplicateFrame {
                    frame_id: id,
                    new_frame_id,
                    index,
                } => {
                    let source = frame_index(doc, &id)?;
                    reserve(used, "frame", &new_frame_id)?;
                    ensure!(
                        doc.frames.len() < schema::MAX_FRAMES
                            && (doc.frames.len() + 1) * doc.layers.len() <= schema::MAX_SLOTS,
                        "frame/slot limit reached"
                    );
                    let at = index.unwrap_or(source + 1);
                    let duration = doc.frames[source].duration_ms;
                    let mut frame = doc.frames[source].clone();
                    frame
                        .annotations
                        .metadata
                        .insert(FRAME_ID_KEY.into(), json!(new_frame_id));
                    let mut entries = Vec::new();
                    for l in 0..doc.layers.len() {
                        let mut entry = doc.layers[l].timeline[source].clone();
                        entry.link = None;
                        if entry.cel.is_some() {
                            let pixels = grid(doc, l, source)?;
                            entry.cel = Some(new_cel(doc, pixels, next_cel));
                        }
                        entries.push(entry);
                    }
                    insert_frame(doc, &new_frame_id, duration, at)?;
                    doc.frames[at] = frame;
                    for (layer, entry) in doc.layers.iter_mut().zip(entries) {
                        layer.timeline.insert(at, entry);
                    }
                }
                FrameOperation::RemoveFrame { frame_id } => {
                    let i = frame_index(doc, &frame_id)?;
                    ensure!(doc.frames.len() > 1, "cannot remove the last frame");
                    doc.frames.remove(i);
                    for layer in &mut doc.layers {
                        layer.timeline.remove(i);
                    }
                }
                FrameOperation::ReorderFrame { frame_id, index } => {
                    let i = frame_index(doc, &frame_id)?;
                    ensure!(index < doc.frames.len(), "frame index out of bounds");
                    let frame = doc.frames.remove(i);
                    doc.frames.insert(index, frame);
                    for layer in &mut doc.layers {
                        let entry = layer.timeline.remove(i);
                        layer.timeline.insert(index, entry);
                    }
                }
                FrameOperation::SetFrameDuration {
                    frame_id,
                    duration_ms,
                } => {
                    let i = frame_index(doc, &frame_id)?;
                    schema::positive(duration_ms, "duration_ms")?;
                    doc.frames[i].duration_ms = duration_ms;
                }
            }
            remap_tags(doc, &before)?;
        }
    }
    Ok(())
}

fn draw(grid: &mut [Vec<u32>], palette_len: usize, drawing: Drawing) -> Result<()> {
    let w = grid[0].len() as i64;
    let h = grid.len() as i64;
    let check = |x: i64, y: i64| -> Result<()> {
        ensure!(
            x >= 0 && y >= 0 && x < w && y < h,
            "coordinates out of bounds"
        );
        Ok(())
    };
    let color = |i: u32| -> Result<()> {
        ensure!((i as usize) < palette_len, "invalid palette_index");
        Ok(())
    };
    match drawing {
        Drawing::SetPixels { pixels } => {
            ensure!(
                pixels.len() <= schema::MAX_PIXELS as usize,
                "too many pixels"
            );
            for p in pixels {
                check(p.x, p.y)?;
                color(p.palette_index)?;
                grid[p.y as usize][p.x as usize] = p.palette_index;
            }
        }
        Drawing::Clear { palette_index } => {
            color(palette_index)?;
            for row in grid {
                row.fill(palette_index);
            }
        }
        Drawing::Rectangle {
            x,
            y,
            width,
            height,
            palette_index,
            filled,
        } => {
            color(palette_index)?;
            check(x, y)?;
            ensure!(
                width > 0 && height > 0,
                "rectangle dimensions must be positive"
            );
            let right = x + i64::from(width) - 1;
            let bottom = y + i64::from(height) - 1;
            check(right, bottom)?;
            for yy in y..=bottom {
                for xx in x..=right {
                    if filled || xx == x || xx == right || yy == y || yy == bottom {
                        grid[yy as usize][xx as usize] = palette_index;
                    }
                }
            }
        }
        Drawing::Line {
            x1,
            y1,
            x2,
            y2,
            palette_index,
        } => {
            check(x1, y1)?;
            check(x2, y2)?;
            color(palette_index)?;
            let (mut x, mut y) = (x1, y1);
            let dx = (x2 - x1).abs();
            let dy = -(y2 - y1).abs();
            let sx = if x1 < x2 { 1 } else { -1 };
            let sy = if y1 < y2 { 1 } else { -1 };
            let mut err = dx + dy;
            loop {
                grid[y as usize][x as usize] = palette_index;
                if x == x2 && y == y2 {
                    break;
                }
                let e = 2 * err;
                if e >= dy {
                    err += dy;
                    x += sx;
                }
                if e <= dx {
                    err += dx;
                    y += sy;
                }
            }
        }
        Drawing::Fill {
            x,
            y,
            palette_index,
        } => {
            check(x, y)?;
            color(palette_index)?;
            let old = grid[y as usize][x as usize];
            if old == palette_index {
                return Ok(());
            }
            let mut pending = vec![(x, y)];
            grid[y as usize][x as usize] = palette_index;
            while let Some((x, y)) = pending.pop() {
                for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                    if nx >= 0
                        && ny >= 0
                        && nx < w
                        && ny < h
                        && grid[ny as usize][nx as usize] == old
                    {
                        grid[ny as usize][nx as usize] = palette_index;
                        pending.push((nx, ny));
                    }
                }
            }
        }
        Drawing::Ellipse {
            x,
            y,
            width,
            height,
            palette_index,
            filled,
        } => {
            check(x, y)?;
            ensure!(
                width > 0 && height > 0,
                "ellipse dimensions must be positive"
            );
            check(x + i64::from(width) - 1, y + i64::from(height) - 1)?;
            color(palette_index)?;
            let ww = i64::from(width);
            let hh = i64::from(height);
            let inside = |xx: i64, yy: i64| {
                if xx < 0 || yy < 0 || xx >= ww || yy >= hh {
                    return false;
                }
                let dx = 2 * xx + 1 - ww;
                let dy = 2 * yy + 1 - hh;
                dx * dx * hh * hh + dy * dy * ww * ww <= ww * ww * hh * hh
            };
            for yy in 0..hh {
                for xx in 0..ww {
                    if inside(xx, yy)
                        && (filled
                            || [(xx - 1, yy), (xx + 1, yy), (xx, yy - 1), (xx, yy + 1)]
                                .into_iter()
                                .any(|(a, b)| !inside(a, b)))
                    {
                        grid[(y + yy) as usize][(x + xx) as usize] = palette_index;
                    }
                }
            }
        }
        Drawing::EraseRegion { .. } | Drawing::ClearCel {} => {
            unreachable!("erase operations are normalized before drawing")
        }
    }
    Ok(())
}
