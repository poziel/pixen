//! Rendered image encoders. All frames are rendered directly from the shared model.
mod animation;
mod svg;

use crate::{model::Sprite, raster, schema};
use anyhow::{Context, Result, ensure};
use image::{ExtendedColorType, ImageEncoder};
use std::{
    io::{self, Seek, SeekFrom, Write},
    ops::Range,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageFormat {
    Png,
    Svg,
    Jpeg,
    Gif,
    Webp,
    Bmp,
    Tiff,
    Apng,
}

#[derive(Clone, Debug, Default)]
pub struct ExportOptions {
    /// One-based source frame. None selects frame 1 or the whole animated timeline.
    pub frame: Option<usize>,
    /// Explicit opaque background, composited before encoding when supplied.
    pub background: Option<[u8; 4]>,
}

pub const MAX_EXPORT_BYTES: u64 = 256 * 1024 * 1024;

fn frames(s: &Sprite, format: ImageFormat, options: &ExportOptions) -> Result<Range<usize>> {
    if let Some(frame) = options.frame {
        ensure!(
            frame >= 1 && frame <= s.frames.len(),
            "frame {frame} is out of range (1..{})",
            s.frames.len()
        );
        return Ok(frame - 1..frame);
    }
    Ok(0..if matches!(
        format,
        ImageFormat::Gif | ImageFormat::Webp | ImageFormat::Apng
    ) {
        s.frames.len()
    } else {
        1
    })
}

/// Validate model, selected frame, alpha, animation timing and SVG complexity before writing.
pub fn validate(s: &Sprite, format: ImageFormat, options: &ExportOptions) -> Result<()> {
    schema::validate_model(s)?;
    ensure!(
        options.background.is_none_or(|bg| bg[3] == 255),
        "background must be opaque (#RRGGBB or #RRGGBBFF)"
    );
    let frames = frames(s, format, options)?;
    if frames.len() > 1 {
        animation::timings(s, format)?;
    }
    for frame in frames {
        if format == ImageFormat::Svg {
            svg::validate(s.width, &rgba(s, frame, options)?)?;
        } else if options.background.is_none()
            && matches!(
                format,
                ImageFormat::Jpeg | ImageFormat::Bmp | ImageFormat::Gif
            )
        {
            let pixels = raster::render(s, frame)?;
            if format == ImageFormat::Gif {
                ensure!(
                    pixels
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .all(|p| p[3] == 0 || p[3] == 255),
                    "GIF cannot preserve partial alpha; supply an explicit opaque --background"
                );
            } else {
                ensure!(
                    pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 255),
                    "{format:?} cannot preserve transparency; supply an explicit opaque --background"
                );
            }
        }
    }
    Ok(())
}

fn rgba(s: &Sprite, frame: usize, options: &ExportOptions) -> Result<Vec<u8>> {
    let mut pixels = raster::render(s, frame)?;
    if let Some(bg) = options.background {
        for p in pixels.as_chunks_mut::<4>().0 {
            let a = u32::from(p[3]);
            for c in 0..3 {
                p[c] = ((u32::from(p[c]) * a + u32::from(bg[c]) * (255 - a) + 127) / 255) as u8;
            }
            p[3] = 255;
        }
    }
    Ok(pixels)
}

/// Write one image, or one complete timeline for GIF/WebP/APNG unless a frame is selected.
/// Encoded output is capped at 256 MiB; callers should prepare temporary outputs before installing.
/// The writer must start at byte zero.
pub fn write<W: Write + Seek>(
    writer: W,
    s: &Sprite,
    format: ImageFormat,
    options: &ExportOptions,
) -> Result<()> {
    validate(s, format, options)?;
    let frames = frames(s, format, options)?;
    let mut writer = BoundedWriter::new(writer, MAX_EXPORT_BYTES)?;
    if frames.len() > 1 {
        animation::write(&mut writer, s, format, options)?;
    } else {
        let pixels = rgba(s, frames.start, options)?;
        let (w, h) = (s.width, s.height);
        match format {
            ImageFormat::Png | ImageFormat::Apng => raster::png(&mut writer, w, h, &pixels)?,
            ImageFormat::Webp => image::codecs::webp::WebPEncoder::new_lossless(&mut writer)
                .write_image(&pixels, w, h, ExtendedColorType::Rgba8)?,
            ImageFormat::Tiff => image::codecs::tiff::TiffEncoder::new(&mut writer).write_image(
                &pixels,
                w,
                h,
                ExtendedColorType::Rgba8,
            )?,
            ImageFormat::Jpeg | ImageFormat::Bmp => {
                let rgb = raster::opaque_rgb(&pixels, None)?;
                if format == ImageFormat::Jpeg {
                    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 95)
                        .write_image(&rgb, w, h, ExtendedColorType::Rgb8)?;
                } else {
                    image::codecs::bmp::BmpEncoder::new(&mut writer).write_image(
                        &rgb,
                        w,
                        h,
                        ExtendedColorType::Rgb8,
                    )?;
                }
            }
            ImageFormat::Gif => animation::gif_still(&mut writer, s.width, s.height, pixels)?,
            ImageFormat::Svg => svg::write(&mut writer, w, h, &pixels)?,
        }
    }
    writer.flush().context("finishing image export")?;
    Ok(())
}

/// Enforces the output budget even for seekable encoders without buffering the full export.
struct BoundedWriter<W> {
    inner: W,
    position: u64,
    end: u64,
    limit: u64,
}
impl<W: Write + Seek> BoundedWriter<W> {
    fn new(mut inner: W, limit: u64) -> io::Result<Self> {
        ensure_start(inner.stream_position()?)?;
        Ok(Self {
            inner,
            position: 0,
            end: 0,
            limit,
        })
    }
}
fn ensure_start(position: u64) -> io::Result<()> {
    if position != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "image export writer must start at offset zero",
        ));
    }
    Ok(())
}
impl<W: Write> Write for BoundedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() as u64 > self.limit.saturating_sub(self.position) {
            return Err(io::Error::other(
                "image export exceeds encoded output byte budget",
            ));
        }
        let n = self.inner.write(bytes)?;
        self.position += n as u64;
        self.end = self.end.max(self.position);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
impl<W: Seek> Seek for BoundedWriter<W> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(n) => i128::from(self.position) + i128::from(n),
            SeekFrom::End(n) => i128::from(self.end) + i128::from(n),
        };
        if position < 0 || position > i128::from(self.limit) {
            return Err(io::Error::other(
                "image export seek exceeds encoded output byte budget",
            ));
        }
        self.position = self.inner.seek(SeekFrom::Start(position as u64))?;
        Ok(self.position)
    }
}
