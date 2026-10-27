use crate::{model::Sprite, raster};
use anyhow::{Context, Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Crop {
    pub x: i64,
    pub y: i64,
    pub width: u32,
    pub height: u32,
}

pub fn load(path: &Path) -> Result<Sprite> {
    ensure!(
        path.is_file(),
        "project file not found or not a regular file: {}; provide an existing supported project",
        path.display()
    );
    match path.extension().and_then(|s|s.to_str()).unwrap_or_default().to_ascii_lowercase().as_str() {
        "json" | "pixloom" | "spritex" => crate::input::compile(path),
        "pxo" | "ase" | "aseprite" => crate::native::read(path),
        "png" | "jpg" | "jpeg" | "gif" | "apng" | "webp" | "bmp" | "tif" | "tiff" => crate::schema::compile(crate::import::document(crate::import::decode(path)?)?),
        _ => anyhow::bail!("unsupported project input; use Pixen JSON/.pixloom, Pixelorama .pxo, RGBA .ase/.aseprite, or PNG/JPEG/GIF/APNG/WebP/BMP/TIFF"),
    }.with_context(|| format!("read preview project {}", path.display()))
}

pub fn png(
    sprite: &Sprite,
    frame: usize,
    crop: Option<Crop>,
    scale: u32,
) -> Result<(Vec<u8>, Crop)> {
    ensure!(scale > 0, "scale must be positive");
    let source = crop.unwrap_or(Crop {
        x: 0,
        y: 0,
        width: sprite.width,
        height: sprite.height,
    });
    ensure!(
        source.x >= 0 && source.y >= 0 && source.width > 0 && source.height > 0,
        "crop origin must be nonnegative and dimensions positive"
    );
    ensure!(
        source.x < i64::from(sprite.width)
            && source.y < i64::from(sprite.height)
            && u64::try_from(source.x)? + u64::from(source.width) <= u64::from(sprite.width)
            && u64::try_from(source.y)? + u64::from(source.height) <= u64::from(sprite.height),
        "crop is outside the canvas"
    );
    let w = source
        .width
        .checked_mul(scale)
        .context("preview width overflow")?;
    let h = source
        .height
        .checked_mul(scale)
        .context("preview height overflow")?;
    ensure!(
        u64::from(w) * u64::from(h) <= 16 * 1024 * 1024,
        "preview exceeds 16 million pixels"
    );
    let composite = raster::render(sprite, frame)?;
    let image = image::RgbaImage::from_raw(sprite.width, sprite.height, composite)
        .context("invalid render")?;
    let region = image::imageops::crop_imm(
        &image,
        source.x as u32,
        source.y as u32,
        source.width,
        source.height,
    )
    .to_image();
    let enlarged = image::imageops::resize(&region, w, h, image::imageops::FilterType::Nearest);
    let mut bytes = Vec::new();
    raster::png(&mut bytes, w, h, enlarged.as_raw())?;
    Ok((bytes, source))
}

pub struct Playback {
    pub frame: usize,
    pub playing: bool,
    elapsed_ms: f64,
}
impl Playback {
    pub fn new(count: usize) -> Self {
        Self {
            frame: 0,
            playing: count > 1,
            elapsed_ms: 0.0,
        }
    }
    pub fn reconcile(&mut self, count: usize) {
        self.frame = self.frame.min(count.saturating_sub(1));
        self.elapsed_ms = 0.0;
        if count <= 1 {
            self.playing = false;
        }
    }
    pub fn select(&mut self, frame: usize, count: usize) {
        self.frame = frame.min(count.saturating_sub(1));
        self.playing = false;
        self.elapsed_ms = 0.0;
    }
    pub fn reconcile_frames(
        &mut self,
        before: &[crate::schema::Frame],
        after: &[crate::schema::Frame],
    ) {
        let selected = before.get(self.frame).and_then(crate::edit::frame_id);
        self.reconcile(after.len());
        if let Some(id) = selected
            && let Some(index) = after
                .iter()
                .position(|f| crate::edit::frame_id(f) == Some(id))
        {
            self.frame = index;
        }
    }
    pub fn stop(&mut self) {
        self.frame = 0;
        self.playing = false;
        self.elapsed_ms = 0.0;
    }
    pub fn toggle(&mut self, count: usize) {
        self.playing = count > 1 && !self.playing;
    }
    pub fn advance(&mut self, delta: Duration, durations: &[f64]) {
        if !self.playing || durations.len() <= 1 {
            return;
        }
        self.frame = self.frame.min(durations.len() - 1);
        let cycle: f64 = durations.iter().sum();
        self.elapsed_ms = (self.elapsed_ms + delta.as_secs_f64() * 1000.0) % cycle;
        while self.elapsed_ms >= durations[self.frame] {
            self.elapsed_ms -= durations[self.frame];
            self.frame = (self.frame + 1) % durations.len();
        }
    }
}

/// Labelled review sheet. Frame composites use exactly the same renderer as exports.
pub fn contact_sheet(
    sprite: &Sprite,
    indices: &[usize],
    scale: u32,
    columns: u32,
) -> Result<(Vec<u8>, serde_json::Value)> {
    use ab_glyph::{Font, FontArc, ScaleFont};
    ensure!(
        !indices.is_empty() && indices.len() <= 64,
        "select 1..64 frames"
    );
    ensure!(
        scale > 0 && columns > 0 && columns <= 64,
        "scale must be positive and columns must be 1..64"
    );

    let font = FontArc::try_from_slice(include_bytes!("../../assets/fonts/Hack-Regular.ttf"))?;
    let scaled = font.as_scaled(16.0);
    let labels: Vec<_> = indices
        .iter()
        .map(|&i| {
            let frame = sprite
                .frames
                .get(i)
                .context("frame index outside timeline")?;
            Ok((
                crate::edit::frame_id(frame)
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("frame-{}", i + 1)),
                format!("{} ms", frame.duration_ms),
            ))
        })
        .collect::<Result<_>>()?;
    let text_width = labels
        .iter()
        .flat_map(|(a, b)| [a, b])
        .map(|s| {
            s.chars()
                .map(|c| scaled.h_advance(font.glyph_id(c)))
                .sum::<f32>()
                .ceil() as u64
        })
        .max()
        .unwrap();
    let image_w = u64::from(sprite.width) * u64::from(scale);
    let image_h = u64::from(sprite.height) * u64::from(scale);
    let cell_w = image_w.max(text_width) + 16;
    let cell_h = image_h + 52;
    let columns = u64::from(columns).min(indices.len() as u64);
    let w = cell_w
        .checked_mul(columns)
        .context("sheet width overflow")?;
    let h = cell_h
        .checked_mul((indices.len() as u64).div_ceil(columns))
        .context("sheet height overflow")?;
    ensure!(
        w.checked_mul(h).is_some_and(|n| n <= 16 * 1024 * 1024),
        "contact sheet exceeds 16 million pixels; reduce scale or select fewer frames"
    );
    let mut sheet =
        image::RgbaImage::from_pixel(w as u32, h as u32, image::Rgba([28, 30, 36, 255]));
    let mut metadata = Vec::new();
    let mut distinct = std::collections::HashSet::new();
    for (slot, &frame) in indices.iter().enumerate() {
        let x = (slot as u64 % columns * cell_w + 8) as u32;
        let y = (slot as u64 / columns * cell_h + 8) as u32;
        let pixels = raster::render(sprite, frame)?;
        distinct.insert(pixels.clone());
        for dy in 0..image_h as u32 {
            for dx in 0..image_w as u32 {
                let offset = (((dy / scale) * sprite.width + dx / scale) * 4) as usize;
                let p = &pixels[offset..offset + 4];
                let bg = if ((dx / 8) + (dy / 8)) % 2 == 0 {
                    78u32
                } else {
                    102u32
                };
                let a = u32::from(p[3]);
                sheet.put_pixel(
                    x + dx,
                    y + dy,
                    image::Rgba([
                        ((u32::from(p[0]) * a + bg * (255 - a) + 127) / 255) as u8,
                        ((u32::from(p[1]) * a + bg * (255 - a) + 127) / 255) as u8,
                        ((u32::from(p[2]) * a + bg * (255 - a) + 127) / 255) as u8,
                        255,
                    ]),
                );
            }
        }
        for (line, text) in [&labels[slot].0, &labels[slot].1].into_iter().enumerate() {
            let mut pen = x as f32;
            let baseline = (y + image_h as u32 + 4 + line as u32 * 18) as f32 + scaled.ascent();
            for c in text.chars() {
                let id = font.glyph_id(c);
                let glyph = id.with_scale_and_position(16.0, ab_glyph::point(pen, baseline));
                if let Some(outline) = font.outline_glyph(glyph) {
                    let bounds = outline.px_bounds();
                    outline.draw(|gx, gy, coverage| {
                        let px = bounds.min.x as i64 + i64::from(gx);
                        let py = bounds.min.y as i64 + i64::from(gy);
                        if px >= 0
                            && py >= 0
                            && px < i64::from(sheet.width())
                            && py < i64::from(sheet.height())
                        {
                            let p = sheet.get_pixel_mut(px as u32, py as u32);
                            for channel in &mut p.0[..3] {
                                *channel = (f32::from(*channel) * (1.0 - coverage)
                                    + 235.0 * coverage)
                                    .round() as u8;
                            }
                        }
                    });
                }
                pen += scaled.h_advance(id);
            }
        }
        metadata.push(serde_json::json!({"frame_id":labels[slot].0,"duration_ms":sprite.frames[frame].duration_ms,"source_frame_index":frame,"image_bounds":{"x":x,"y":y,"width":image_w,"height":image_h}}));
    }
    let mut bytes = Vec::new();
    raster::png(&mut bytes, w as u32, h as u32, sheet.as_raw())?;
    Ok((
        bytes,
        serde_json::json!({"width":sprite.width,"height":sprite.height,"scale":scale,"sheet_width":w,"sheet_height":h,"frames":metadata,"distinct_composites":distinct.len()}),
    ))
}
