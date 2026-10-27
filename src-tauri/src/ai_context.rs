//! App-owned per-message snapshots. References never authorize reading arbitrary native paths.
use anyhow::{ensure, Context, Result};
use base64::Engine;
use pixen::mcp::desktop::Desktop;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub name: String,
    pub data: String,
}
#[derive(Serialize)]
pub struct InputImage {
    pub name: String,
    pub path: String,
    pub data: String,
}
pub struct TurnContext {
    pub images: Vec<InputImage>,
    pub notes: String,
}

fn image(name: String, source: &Path, destination: &Path, enlarge: bool) -> Result<InputImage> {
    let mut reader = image::ImageReader::open(source)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(128 * 1024 * 1024);
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    reader.limits(limits);
    let decoded = reader.decode().context("Cannot decode reference image")?;
    let decoded = if decoded.width() > 2048 || decoded.height() > 2048 {
        decoded.resize(2048, 2048, image::imageops::FilterType::Nearest)
    } else if enlarge {
        let factor = (512 / decoded.width().max(decoded.height())).clamp(1, 16);
        decoded.resize_exact(
            decoded.width() * factor,
            decoded.height() * factor,
            image::imageops::FilterType::Nearest,
        )
    } else {
        decoded
    };
    decoded.save_with_format(destination, image::ImageFormat::Png)?;
    let bytes = std::fs::read(destination)?;
    ensure!(
        bytes.len() <= 16 * 1024 * 1024,
        "Normalized reference image exceeds 16 MiB"
    );
    Ok(InputImage {
        name,
        path: destination.to_string_lossy().into_owned(),
        data: base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

pub fn prepare(desktop: &Desktop, cwd: &Path, references: &[Reference]) -> Result<TurnContext> {
    ensure!(
        references.len() <= 8,
        "Attach at most eight reference files per message"
    );
    ensure!(
        references.iter().map(|r| r.data.len()).sum::<usize>() <= 32 * 1024 * 1024,
        "References exceed 24 MiB"
    );
    let directory = tempfile::Builder::new().prefix("turn-").tempdir_in(cwd)?;
    let (revision, mut sprite, _) = desktop.snapshot(None).context("No artwork is open")?;
    let frame_count = sprite.frames.len();
    sprite.frames.truncate(16);
    let columns = (sprite.frames.len() as f64).sqrt().ceil() as u32;
    let rows = (sprite.frames.len() as u32).div_ceil(columns);
    let cell_limit = 2048 / columns.max(rows).max(1);
    let ratio = (sprite.width as f64 / cell_limit as f64)
        .max(sprite.height as f64 / cell_limit as f64)
        .max(1.0);
    let tile_width = ((sprite.width as f64 / ratio).floor() as u32).max(1);
    let tile_height = ((sprite.height as f64 / ratio).floor() as u32).max(1);
    let mut sheet = image::RgbaImage::new(tile_width * columns, tile_height * rows);
    for index in 0..sprite.frames.len() {
        let pixels = pixen::raster::render(&sprite, index)?;
        let source = image::RgbaImage::from_raw(sprite.width, sprite.height, pixels)
            .context("Invalid artwork pixels")?;
        let tile = image::imageops::resize(
            &source,
            tile_width,
            tile_height,
            image::imageops::FilterType::Nearest,
        );
        image::imageops::replace(
            &mut sheet,
            &tile,
            ((index as u32 % columns) * tile_width) as i64,
            ((index as u32 / columns) * tile_height) as i64,
        );
    }
    let raw = directory.path().join("artwork-source.png");
    sheet.save_with_format(&raw, image::ImageFormat::Png)?;
    let mut images = vec![image(
        "Current artwork".into(),
        &raw,
        &directory.path().join("artwork.png"),
        true,
    )?];
    let mut notes=format!("Image 1 is the CURRENT artwork at message-send time, revision {revision}: {} × {} pixels per frame, {} frame(s) total. The overview shows the first {} frame(s), row-major in {columns} column(s), including transparent pixels, scaled with nearest-neighbor for visibility. Use MCP to inspect full-resolution pixels or additional frames.\n",sprite.width,sprite.height,frame_count,sprite.frames.len());
    for (index, reference) in references.iter().enumerate() {
        ensure!(
            reference.data.len() <= 24 * 1024 * 1024,
            "Reference file exceeds 16 MiB"
        );
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&reference.data)
            .context("Invalid reference data")?;
        ensure!(
            bytes.len() <= 16 * 1024 * 1024,
            "Reference file exceeds 16 MiB"
        );
        let name = reference.name.chars().take(256).collect::<String>();
        let path = directory.path().join(format!("input-{index}"));
        std::fs::write(&path, &bytes)?;
        let reference = pixen::agent::attachments::Attachment::load(&path)?;
        if let Some(text) = reference.text {
            notes.push_str(&format!(
                "\n[User reference document: {name}]\n{text}\n[End reference document]\n"
            ));
        } else {
            let prepared = image(
                name.clone(),
                &reference.path,
                &directory.path().join(format!("reference-{index}.png")),
                false,
            )?;
            notes.push_str(&format!(
                "Image {} is a USER REFERENCE: {name}. It is not the current artwork.\n",
                images.len() + 1
            ));
            images.push(prepared);
        }
    }
    // Keep snapshots available for native providers throughout the conversation.
    let _ = directory.keep();
    Ok(TurnContext { images, notes })
}
