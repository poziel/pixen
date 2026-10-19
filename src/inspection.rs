//! Shared, pixel-exact AI and desktop inspection renders. These never edit artwork.
use crate::{model::Sprite, raster};
use anyhow::{Context, Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Composite,
    OnionSkin,
    FrameComparison,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub frame_id: String,
    pub reference_frame_id: Option<String>,
    pub mode: Mode,
    pub layer_id: Option<String>,
}

pub fn render(
    sprite: &Sprite,
    frame: usize,
    reference: Option<usize>,
    mode: Mode,
) -> Result<(Vec<u8>, Option<usize>)> {
    render_with(sprite.frames.len(), frame, reference, mode, |i| {
        raster::render(sprite, i)
    })
}
/// Differences use rendered RGBA, counting each pixel once (including alpha changes).
/// Onion skin uses red previous/explicit reference and cyan next; no timeline wrapping.
pub fn render_with(
    count: usize,
    frame: usize,
    reference: Option<usize>,
    mode: Mode,
    mut render: impl FnMut(usize) -> Result<Vec<u8>>,
) -> Result<(Vec<u8>, Option<usize>)> {
    ensure!(
        frame < count && reference.is_none_or(|i| i < count),
        "inspection frame out of range"
    );
    let current = render(frame)?;
    if mode == Mode::Composite {
        return Ok((current, None));
    }
    if mode == Mode::FrameComparison {
        let index = reference.or_else(|| frame.checked_sub(1)).unwrap_or(frame);
        let other = render(index)?;
        let mut out = current.clone();
        let mut changed = 0;
        for ((a, b), dst) in current
            .as_chunks::<4>()
            .0
            .iter()
            .zip(other.as_chunks::<4>().0.iter())
            .zip(out.as_chunks_mut::<4>().0.iter_mut())
        {
            if a != b {
                changed += 1;
                dst.copy_from_slice(&[255, 64, 180, 255]);
            } else {
                dst[3] = ((u16::from(a[3]) * 60) / 255) as u8;
            }
        }
        return Ok((out, Some(changed)));
    }
    let mut out = vec![0; current.len()];
    for (index, tint) in [
        (reference.or_else(|| frame.checked_sub(1)), [255, 80, 80]),
        (
            if reference.is_none() && frame + 1 < count {
                Some(frame + 1)
            } else {
                None
            },
            [60, 210, 255],
        ),
    ] {
        if let Some(index) = index {
            let ghost = render(index)?;
            for (dst, src) in out
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(ghost.as_chunks::<4>().0.iter())
            {
                over(
                    dst,
                    &[
                        tint[0],
                        tint[1],
                        tint[2],
                        ((u16::from(src[3]) * 90) / 255) as u8,
                    ],
                );
            }
        }
    }
    for (dst, src) in out
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(current.as_chunks::<4>().0.iter())
    {
        over(dst, src);
    }
    Ok((out, None))
}
fn over(dst: &mut [u8], src: &[u8]) {
    let sa = u32::from(src[3]);
    let da = u32::from(dst[3]);
    let alpha = sa * 255 + da * (255 - sa);
    if alpha == 0 {
        dst.fill(0);
        return;
    }
    for i in 0..3 {
        dst[i] = ((u32::from(src[i]) * sa * 255 + u32::from(dst[i]) * da * (255 - sa) + alpha / 2)
            / alpha) as u8;
    }
    dst[3] = ((alpha + 127) / 255) as u8;
}
pub fn png(width: u32, height: u32, rgba: &[u8], scale: u32) -> Result<Vec<u8>> {
    ensure!(scale > 0, "scale must be positive");
    let w = width.checked_mul(scale).context("preview width overflow")?;
    let h = height
        .checked_mul(scale)
        .context("preview height overflow")?;
    ensure!(
        u64::from(w) * u64::from(h) <= 16 * 1024 * 1024,
        "preview exceeds 16 million pixels"
    );
    let image = image::RgbaImage::from_raw(width, height, rgba.to_vec())
        .context("invalid inspection render")?;
    let image = image::imageops::resize(&image, w, h, image::imageops::FilterType::Nearest);
    let mut png = Vec::new();
    raster::png(&mut png, w, h, image.as_raw())?;
    Ok(png)
}
