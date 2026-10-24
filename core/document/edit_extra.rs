//! Palette-index-preserving edits and bounded, snapshot-based region operations.
use crate::{
    edit::{self, DrawOperation, Drawing, Operation, Pixel},
    preview::Crop,
    schema::Document,
};
use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PaletteOperation {
    AppendColors {
        colors: Vec<String>,
    },
    SetColor {
        palette_index: u32,
        color: String,
    },
    RemapColor {
        from_index: u32,
        to_index: u32,
        frame_id: Option<String>,
        layer_id: Option<String>,
    },
}
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Transform {
    #[default]
    None,
    FlipHorizontal,
    FlipVertical,
    Rotate90,
    Rotate180,
    Rotate270,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CopyRegion {
    pub source_frame_id: String,
    pub source_layer_id: String,
    pub region: Crop,
    pub frame_id: String,
    pub layer_id: String,
    pub x: i64,
    pub y: i64,
    #[serde(default)]
    pub transform: Transform,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransformRegion {
    pub frame_id: String,
    pub layer_id: String,
    pub region: Crop,
    pub x: i64,
    pub y: i64,
    pub transform: Transform,
}
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RegionOperation {
    CopyRegion(CopyRegion),
    TransformRegion(TransformRegion),
}
fn bounds(doc: &Document, r: Crop) -> Result<()> {
    ensure!(
        r.x >= 0
            && r.y >= 0
            && r.width > 0
            && r.height > 0
            && r.x as u64 + u64::from(r.width) <= u64::from(doc.canvas.width)
            && r.y as u64 + u64::from(r.height) <= u64::from(doc.canvas.height),
        "region must lie wholly within the canvas"
    );
    Ok(())
}
fn draw(
    doc: &mut Document,
    frame_id: &str,
    layer_id: &str,
    drawing: Drawing,
    used: &mut BTreeSet<String>,
    next: &mut u64,
) -> Result<()> {
    edit::execute(
        doc,
        Operation::Draw(DrawOperation {
            frame_id: frame_id.into(),
            layer_id: layer_id.into(),
            drawing,
        }),
        used,
        next,
    )
}
pub fn palette(
    doc: &mut Document,
    op: PaletteOperation,
    used: &mut BTreeSet<String>,
    next: &mut u64,
) -> Result<()> {
    match op {
        PaletteOperation::AppendColors { colors } => {
            ensure!(
                !colors.is_empty() && doc.palette.len() + colors.len() <= 1_048_576,
                "palette must contain at most 1048576 colors; append at least one"
            );
            for c in &colors {
                crate::schema::decode_color(c)?;
            }
            doc.palette.extend(colors);
        }
        PaletteOperation::SetColor {
            palette_index,
            color,
        } => {
            crate::schema::decode_color(&color)?;
            ensure!(
                (palette_index as usize) < doc.palette.len(),
                "palette_index is out of range"
            );
            doc.palette[palette_index as usize] = color;
            edit::transparent(doc);
        }
        PaletteOperation::RemapColor {
            from_index,
            to_index,
            frame_id,
            layer_id,
        } => {
            ensure!(
                (from_index as usize) < doc.palette.len()
                    && (to_index as usize) < doc.palette.len(),
                "palette index is out of range"
            );
            if let Some(id) = &frame_id {
                edit::frame_index(doc, id)?;
            }
            if let Some(id) = &layer_id {
                ensure!(doc.layers.iter().any(|l| &l.id == id), "unknown layer_id");
            }
            // Build targets before writing so links are remapped exactly once from original values.
            let mut edits = Vec::new();
            for (li, layer) in doc.layers.iter().enumerate() {
                if layer_id.as_ref().is_some_and(|id| id != &layer.id) {
                    continue;
                }
                for (fi, frame) in doc.frames.iter().enumerate() {
                    let id = edit::frame_id(frame).unwrap();
                    if frame_id.as_ref().is_some_and(|target| target != id) {
                        continue;
                    }
                    let grid = edit::grid(doc, li, fi)?;
                    let mut pixels = Vec::new();
                    for (y, row) in grid.iter().enumerate() {
                        for (x, &index) in row.iter().enumerate() {
                            if index == from_index {
                                pixels.push(Pixel {
                                    x: x as i64,
                                    y: y as i64,
                                    palette_index: to_index,
                                });
                            }
                        }
                    }
                    if !pixels.is_empty() {
                        edits.push((id.to_owned(), layer.id.clone(), pixels));
                    }
                }
            }
            for (f, l, pixels) in edits {
                draw(doc, &f, &l, Drawing::SetPixels { pixels }, used, next)?;
            }
        }
    }
    Ok(())
}
pub fn region(
    doc: &mut Document,
    op: RegionOperation,
    used: &mut BTreeSet<String>,
    next: &mut u64,
) -> Result<()> {
    let moving = matches!(op, RegionOperation::TransformRegion(_));
    let copy = match op {
        RegionOperation::CopyRegion(c) => c,
        RegionOperation::TransformRegion(t) => CopyRegion {
            source_frame_id: t.frame_id.clone(),
            source_layer_id: t.layer_id.clone(),
            region: t.region,
            frame_id: t.frame_id,
            layer_id: t.layer_id,
            x: t.x,
            y: t.y,
            transform: t.transform,
        },
    };
    bounds(doc, copy.region)?;
    let quarter = matches!(copy.transform, Transform::Rotate90 | Transform::Rotate270);
    let (w, h) = (copy.region.width, copy.region.height);
    bounds(
        doc,
        Crop {
            x: copy.x,
            y: copy.y,
            width: if quarter { h } else { w },
            height: if quarter { w } else { h },
        },
    )?;
    let li = doc
        .layers
        .iter()
        .position(|l| l.id == copy.source_layer_id)
        .ok_or_else(|| anyhow::anyhow!("unknown source_layer_id"))?;
    let fi = edit::frame_index(doc, &copy.source_frame_id)?;
    let grid = edit::grid(doc, li, fi)?;
    let mut pixels = Vec::with_capacity(w as usize * h as usize);
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = match copy.transform {
                Transform::None => (x, y),
                Transform::FlipHorizontal => (w - 1 - x, y),
                Transform::FlipVertical => (x, h - 1 - y),
                Transform::Rotate90 => (h - 1 - y, x),
                Transform::Rotate180 => (w - 1 - x, h - 1 - y),
                Transform::Rotate270 => (y, w - 1 - x),
            };
            pixels.push(Pixel {
                x: copy.x + i64::from(dx),
                y: copy.y + i64::from(dy),
                palette_index: grid[(copy.region.y + i64::from(y)) as usize]
                    [(copy.region.x + i64::from(x)) as usize],
            });
        }
    }
    if moving {
        draw(
            doc,
            &copy.source_frame_id,
            &copy.source_layer_id,
            Drawing::EraseRegion {
                x: copy.region.x,
                y: copy.region.y,
                width: w,
                height: h,
            },
            used,
            next,
        )?;
    }
    draw(
        doc,
        &copy.frame_id,
        &copy.layer_id,
        Drawing::SetPixels { pixels },
        used,
        next,
    )
}
