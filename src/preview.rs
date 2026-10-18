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
        _ => anyhow::bail!("unsupported project input; use Pixloom JSON/.pixloom, Pixelorama .pxo, or RGBA .ase/.aseprite"),
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
