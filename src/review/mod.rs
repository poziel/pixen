//! Deterministic, read-only evidence for art review. No aesthetic scoring or model calls.
pub mod animation;
pub mod checkpoints;
pub mod compare;
pub mod measure;
pub mod render;
pub mod rules;
use crate::{preview::Crop, schema::Document};
use anyhow::{Context, Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
pub const MAX_IMAGE_PIXELS: u64 = 16 * 1024 * 1024;
pub const MAX_WORK: u64 = 64 * 1024 * 1024;
pub const MAX_RECORDS: usize = 256;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Representation {
    #[default]
    Color,
    Silhouette,
    Grayscale,
}
#[derive(Clone, Debug)]
pub struct ReviewImage {
    pub role: String,
    pub png: Vec<u8>,
}
#[derive(Clone, Debug)]
pub struct ReviewResult {
    pub report: Value,
    pub images: Vec<ReviewImage>,
}
impl ReviewResult {
    pub fn report(report: Value) -> Self {
        Self {
            report,
            images: Vec::new(),
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub frame_ids: Option<Vec<String>>,
    pub layer_ids: Option<Vec<String>>,
    pub crop: Option<Crop>,
}
pub struct ResolvedScope {
    pub frames: Vec<usize>,
    pub layers: Vec<usize>,
    pub crop: Crop,
}
pub fn crop(width: u32, height: u32, region: Option<Crop>) -> Result<Crop> {
    let r = region.unwrap_or(Crop {
        x: 0,
        y: 0,
        width,
        height,
    });
    ensure!(
        r.x >= 0
            && r.y >= 0
            && r.width > 0
            && r.height > 0
            && r.x as u64 + u64::from(r.width) <= u64::from(width)
            && r.y as u64 + u64::from(r.height) <= u64::from(height),
        "crop must lie wholly inside the canvas"
    );
    Ok(r)
}
pub fn indices(ids: Option<&[String]>, all: &[String], limit: usize) -> Result<Vec<usize>> {
    let out = if let Some(ids) = ids {
        ensure!(!ids.is_empty(), "selection must not be empty");
        let mut seen = std::collections::BTreeSet::new();
        ids.iter()
            .map(|id| {
                ensure!(seen.insert(id), "selection IDs must be unique");
                all.iter()
                    .position(|s| s == id)
                    .context("unknown selection ID")
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        (0..all.len()).collect()
    };
    ensure!(
        out.len() <= limit,
        "too many selected targets; select at most {limit}"
    );
    Ok(out)
}
pub fn frame_keys(frames: &[crate::schema::Frame]) -> Vec<String> {
    let mut used = frames
        .iter()
        .filter_map(crate::edit::frame_id)
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>();
    frames
        .iter()
        .enumerate()
        .map(|(i, f)| {
            if let Some(id) = crate::edit::frame_id(f) {
                id.to_owned()
            } else {
                let mut n = i + 1;
                while used.contains(&format!("frame-{n}")) {
                    n += 1;
                }
                let id = format!("frame-{n}");
                used.insert(id.clone());
                id
            }
        })
        .collect()
}
pub fn frame_ids(doc: &Document) -> Vec<String> {
    frame_keys(&doc.frames)
}
pub fn validate_scope(doc: &Document, scope: &Scope) -> Result<ResolvedScope> {
    let frames = indices(scope.frame_ids.as_deref(), &frame_ids(doc), 64)?;
    let layers = indices(
        scope.layer_ids.as_deref(),
        &doc.layers.iter().map(|l| l.id.clone()).collect::<Vec<_>>(),
        crate::schema::MAX_LAYERS,
    )?;
    let crop = crop(doc.canvas.width, doc.canvas.height, scope.crop)?;
    // Compilation and compositing visit full source canvases, even for a small crop.
    ensure!(
        doc.canvas.width as u64 * doc.canvas.height as u64 * frames.len() as u64 <= MAX_WORK,
        "review exceeds 64 million source pixels; select fewer frames"
    );
    Ok(ResolvedScope {
        frames,
        layers,
        crop,
    })
}
pub fn stamp(result: &mut ReviewResult, id: &str, revision: u64, source_identity: u64) {
    result.report["report_version"] = json!(1);
    result.report["project_id"] = json!(id);
    result.report["inspected_revision"] = json!(revision);
    result.report["source_identity"] = json!(source_identity);
    if result.report.get("findings").is_none() {
        result.report["findings"] = json!([]);
    }
    if result.report.get("truncation").is_none() {
        result.report["truncation"] = json!({});
    }
}
pub fn identity(doc: &Document) -> Result<u64> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_vec(doc)?.hash(&mut h);
    Ok(h.finish())
}
pub fn document_for(sprite: &crate::model::Sprite) -> Result<Document> {
    let mut doc = crate::schema::to_document(sprite)?;
    let ids = frame_ids(&doc);
    for (f, id) in doc.frames.iter_mut().zip(ids) {
        if crate::edit::frame_id(f).is_none() {
            f.annotations
                .metadata
                .insert(crate::edit::FRAME_ID_KEY.into(), json!(id));
        }
    }
    Ok(doc)
}
