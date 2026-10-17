use crate::model::Sprite;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Annotations {
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub user_data: String,
}

/// Optional native swatches and credits, carried by unpack as data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PixeloramaExtras {
    #[serde(default)]
    pub palettes: Vec<BTreeMap<String, String>>,
    #[serde(default)]
    pub project_current_palette_name: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub author_display_name: String,
    #[serde(default)]
    pub author_real_name: String,
    #[serde(default)]
    pub author_contact: String,
    #[serde(default)]
    pub author_company: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub version: u32,
    pub canvas: Canvas,
    pub palette: Vec<String>,
    pub cels: Vec<CelDefinition>,
    #[serde(default = "default_fps")]
    pub fps: f64,
    pub frames: Vec<Frame>,
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub tags: Vec<Tag>,
    #[serde(default)]
    pub annotations: Annotations,
    #[serde(default)]
    pub pixelorama: PixeloramaExtras,
}
fn default_fps() -> f64 {
    12.0
}
fn one() -> f64 {
    1.0
}
fn yes() -> bool {
    true
}
fn tag_color() -> String {
    "#6699CCFF".into()
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CelDefinition {
    pub id: String,
    pub pixels: Vec<Vec<u32>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub duration_ms: f64,
    #[serde(default)]
    pub annotations: Annotations,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub id: String,
    pub name: String,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default = "one")]
    pub opacity: f64,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub new_cels_linked: bool,
    pub timeline: Vec<Entry>,
    #[serde(default)]
    pub annotations: Annotations,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    #[serde(deserialize_with = "required_cel")]
    pub cel: Option<String>,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default = "one")]
    pub opacity: f64,
    #[serde(default)]
    pub annotations: Annotations,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tag {
    pub name: String,
    pub from: usize,
    pub to: usize,
    #[serde(default = "tag_color")]
    pub color: String,
    #[serde(default)]
    pub user_data: String,
}
fn required_cel<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}

pub const MAX_JSON: u64 = 256 * 1024 * 1024;
pub const MAX_PIXELS: u64 = 4 * 1024 * 1024;
pub const MAX_IMAGE_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_GRID_PIXELS: usize = 64 * 1024 * 1024;
pub const MAX_FRAMES: usize = 4096;
pub const MAX_LAYERS: usize = 256;
pub const MAX_SLOTS: usize = 32768;

pub fn pixel_bytes(width: u32, height: u32) -> Result<usize> {
    let n = u64::from(width) * u64::from(height);
    ensure!(
        width > 0 && height > 0 && width <= 4096 && height <= 4096 && n <= MAX_PIXELS,
        "canvas must be 1..4096 per side and at most {MAX_PIXELS} pixels"
    );
    Ok((n * 4) as usize)
}
pub fn opacity(value: f64) -> Result<()> {
    ensure!(
        value.is_finite() && (0.0..=1.0).contains(&value),
        "opacity must be finite and in [0, 1]"
    );
    Ok(())
}
pub fn positive(value: f64, name: &str) -> Result<()> {
    ensure!(
        value.is_finite() && value > 0.0,
        "{name} must be finite and positive"
    );
    Ok(())
}
pub fn identifier(s: &str) -> Result<()> {
    ensure!(
        !s.is_empty()
            && s.len() <= 128
            && s.bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-'),
        "identifier {s:?} must contain 1..128 ASCII letters, digits, underscores or hyphens"
    );
    Ok(())
}
pub fn decode_color(s: &str) -> Result<[u8; 4]> {
    ensure!(
        s.len() == 9 && s.starts_with('#') && s.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit),
        "colour {s:?} must be #RRGGBBAA"
    );
    let mut rgba = [0; 4];
    for (i, byte) in rgba.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[1 + i * 2..3 + i * 2], 16)?;
    }
    Ok(rgba)
}
pub fn parse(bytes: &[u8]) -> Result<Sprite> {
    ensure!(
        bytes.len() as u64 <= MAX_JSON,
        "authoring JSON exceeds {MAX_JSON} bytes"
    );
    let doc: Document = serde_json::from_slice(bytes).context("invalid sprite JSON")?;
    compile(doc)
}
pub fn compile(doc: Document) -> Result<Sprite> {
    ensure!(
        doc.version == 1,
        "unsupported authoring version {}; expected 1",
        doc.version
    );
    let len = pixel_bytes(doc.canvas.width, doc.canvas.height)?;
    ensure!(
        !doc.palette.is_empty() && doc.palette.len() <= 1_048_576,
        "palette must contain 1..1048576 colours"
    );
    let palette: Vec<_> = doc
        .palette
        .iter()
        .map(|s| decode_color(s))
        .collect::<Result<_>>()?;
    let mut images = BTreeMap::new();
    ensure!(doc.cels.len() <= MAX_SLOTS, "too many cel definitions");
    ensure!(
        doc.cels
            .len()
            .checked_mul(len / 4)
            .is_some_and(|n| n <= MAX_GRID_PIXELS),
        "cel grids exceed pixel budget"
    );
    ensure!(
        doc.cels
            .len()
            .checked_mul(len)
            .is_some_and(|n| n <= MAX_IMAGE_BYTES),
        "cel buffers exceed memory budget"
    );
    for cel in doc.cels {
        identifier(&cel.id)?;
        ensure!(!images.contains_key(&cel.id), "duplicate cel id {}", cel.id);
        ensure!(
            cel.pixels.len() == doc.canvas.height as usize,
            "cel {}: wrong grid height",
            cel.id
        );
        let mut pixels = Vec::with_capacity(len);
        for row in cel.pixels {
            ensure!(
                row.len() == doc.canvas.width as usize,
                "cel {}: wrong grid width",
                cel.id
            );
            for index in row {
                pixels.extend_from_slice(palette.get(index as usize).with_context(|| {
                    format!("cel {}: palette index {index} is out of range", cel.id)
                })?);
            }
        }
        images.insert(cel.id, Arc::from(pixels));
    }
    let sprite = Sprite {
        width: doc.canvas.width,
        height: doc.canvas.height,
        fps: doc.fps,
        annotations: doc.annotations,
        pixelorama: doc.pixelorama,
        frames: doc.frames,
        layers: doc.layers,
        tags: doc.tags,
        images,
    };
    validate_model(&sprite)?;
    Ok(sprite)
}
pub fn validate_model(s: &Sprite) -> Result<()> {
    let bytes = pixel_bytes(s.width, s.height)?;
    positive(s.fps, "fps")?;
    ensure!(
        !s.frames.is_empty() && s.frames.len() <= MAX_FRAMES,
        "frame count must be 1..{MAX_FRAMES}"
    );
    ensure!(
        !s.layers.is_empty() && s.layers.len() <= MAX_LAYERS,
        "layer count must be 1..{MAX_LAYERS}"
    );
    ensure!(
        s.layers.len() * s.frames.len() <= MAX_SLOTS,
        "too many timeline cels"
    );
    let mut total = 0.0;
    for f in &s.frames {
        positive(f.duration_ms, "duration_ms")?;
        positive(f.duration_ms / 1000.0 * s.fps, "PXO duration multiplier")?;
        total += f.duration_ms;
    }
    ensure!(total.is_finite(), "total duration overflow");
    let mut ids = HashSet::new();
    validate_annotations(&s.annotations)?;
    for entry in &s.pixelorama.palettes {
        ensure!(
            entry.len() == 1,
            "native palette entry must have exactly one name"
        );
        for (name, encoded) in entry {
            ensure!(!name.is_empty(), "native palette name is empty");
            let value: serde_json::Value =
                serde_json::from_str(encoded).context("native palette is not JSON")?;
            ensure!(
                value.is_object(),
                "native palette must contain a JSON object"
            );
        }
    }
    for f in &s.frames {
        validate_annotations(&f.annotations)?;
    }
    for l in &s.layers {
        validate_annotations(&l.annotations)?;
        identifier(&l.id)?;
        ensure!(ids.insert(&l.id), "duplicate layer id {}", l.id);
        ensure!(!l.name.trim().is_empty(), "layer name is empty");
        opacity(l.opacity)?;
        ensure!(
            l.timeline.len() == s.frames.len(),
            "layer {} timeline must contain exactly {} entries",
            l.id,
            s.frames.len()
        );
        let mut links = HashMap::new();
        for e in &l.timeline {
            validate_annotations(&e.annotations)?;
            opacity(e.opacity)?;
            if let Some(c) = &e.cel {
                ensure!(s.images.contains_key(c), "layer {}: unknown cel {c}", l.id);
            }
            if let Some(link) = &e.link {
                identifier(link)?;
                if let Some(prev) = links.insert(link, &e.cel) {
                    ensure!(
                        prev == &e.cel,
                        "layer {} link {link} references different cel definitions",
                        l.id
                    );
                }
            }
        }
    }
    let mut names = HashSet::new();
    for t in &s.tags {
        ensure!(
            !t.name.trim().is_empty() && names.insert(&t.name),
            "empty or duplicate tag name"
        );
        ensure!(
            t.from >= 1 && t.from <= t.to && t.to <= s.frames.len(),
            "tag {}: invalid inclusive frame range",
            t.name
        );
        decode_color(&t.color)?;
    }
    for (id, image) in &s.images {
        identifier(id)?;
        ensure!(image.len() == bytes, "cel {id}: invalid RGBA byte count");
    }
    Ok(())
}
fn validate_annotations(a: &Annotations) -> Result<()> {
    ensure!(
        !a.metadata.keys().any(|k| k.starts_with("_spritec")),
        "metadata keys beginning _spritec are reserved"
    );
    Ok(())
}

pub fn to_document(s: &Sprite) -> Result<Document> {
    validate_model(s)?;
    let mut palette = Vec::new();
    let mut indices = HashMap::<[u8; 4], u32>::new();
    let mut cels = Vec::new();
    let mut total = 0usize;
    for (id, bytes) in &s.images {
        total = total
            .checked_add(bytes.len() / 4)
            .context("grid size overflow")?;
        ensure!(
            total <= MAX_GRID_PIXELS,
            "unpacked grids exceed pixel budget"
        );
        let mut pixels = Vec::new();
        for row in bytes.chunks_exact(s.width as usize * 4) {
            let mut output = Vec::with_capacity(s.width as usize);
            for p in row.as_chunks::<4>().0 {
                let rgba = *p;
                let next = palette.len() as u32;
                let index = *indices.entry(rgba).or_insert_with(|| {
                    palette.push(format!("#{:02X}{:02X}{:02X}{:02X}", p[0], p[1], p[2], p[3]));
                    next
                });
                output.push(index);
            }
            pixels.push(output);
        }
        cels.push(CelDefinition {
            id: id.clone(),
            pixels,
        });
    }
    if palette.is_empty() {
        palette.push("#00000000".into());
    }
    ensure!(
        palette.len() <= 1_048_576,
        "unpacked palette exceeds 1048576 colours"
    );
    Ok(Document {
        version: 1,
        canvas: Canvas {
            width: s.width,
            height: s.height,
        },
        palette,
        cels,
        fps: s.fps,
        frames: s.frames.clone(),
        layers: s.layers.clone(),
        tags: s.tags.clone(),
        annotations: s.annotations.clone(),
        pixelorama: s.pixelorama.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_numbers_reject_nonfinite_values() {
        for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(positive(n, "timing").is_err());
            assert!(opacity(n).is_err());
        }
        for n in [0.0, -1.0] {
            assert!(positive(n, "timing").is_err());
        }
        assert!(positive(0.125, "timing").is_ok());
        assert!(opacity(0.0).is_ok());
        assert!(opacity(1.0).is_ok());
    }
    #[test]
    fn canvas_bounds_are_checked_before_allocating_pixels() {
        assert_eq!(pixel_bytes(4096, 1024).unwrap(), 16 * 1024 * 1024);
        for (w, h) in [
            (0, 1),
            (1, 0),
            (4097, 1),
            (4096, 4096),
            (u32::MAX, u32::MAX),
        ] {
            assert!(pixel_bytes(w, h).is_err());
        }
    }
}
