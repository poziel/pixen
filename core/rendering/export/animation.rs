use super::{ExportOptions, ImageFormat, MAX_EXPORT_BYTES, rgba};
use crate::model::Sprite;
use anyhow::{Result, ensure};
use image::{ExtendedColorType, ImageEncoder};
use std::io::{Cursor, Seek, SeekFrom, Write};

pub(super) struct Timing {
    pub ticks: Vec<u32>,
    pub denominator: u16,
}

pub(super) fn timings(s: &Sprite, format: ImageFormat) -> Result<Timing> {
    let (denominator, maximum) = match format {
        ImageFormat::Gif => (100, 65535),
        ImageFormat::Webp => (1000, 0xFF_FFFF),
        ImageFormat::Apng => {
            let max = s.frames.iter().map(|f| f.duration_ms).fold(0.0, f64::max);
            let den = [10000, 1000, 100, 10, 1]
                .into_iter()
                .find(|d| max / 1000.0 * f64::from(*d) <= 65535.0)
                .ok_or_else(|| anyhow::anyhow!("APNG frame duration exceeds 65535 seconds"))?;
            (den, 65535)
        }
        _ => unreachable!("only animated formats have timings"),
    };
    let mut cumulative = 0.0;
    let mut previous = 0u64;
    let mut ticks = Vec::with_capacity(s.frames.len());
    for (i, frame) in s.frames.iter().enumerate() {
        cumulative += frame.duration_ms / 1000.0 * f64::from(denominator);
        ensure!(
            cumulative.is_finite() && cumulative < u64::MAX as f64,
            "{format:?} animation duration overflow"
        );
        let endpoint = cumulative.round() as u64;
        let duration = endpoint.saturating_sub(previous);
        ensure!(
            duration > 0,
            "{format:?} frame {} rounds to zero duration at 1/{denominator} second precision; increase its duration",
            i + 1
        );
        ensure!(
            duration <= maximum,
            "{format:?} frame {} duration exceeds its {maximum}-tick limit",
            i + 1
        );
        ticks.push(duration as u32);
        previous = endpoint;
    }
    Ok(Timing { ticks, denominator })
}

fn gif_frame(width: u32, height: u32, mut pixels: Vec<u8>) -> gif::Frame<'static> {
    let mut frame = gif::Frame::from_rgba_speed(width as u16, height as u16, &mut pixels, 10);
    // NeuQuant can map opaque pixels to its transparent palette entry. Reserve that
    // entry strictly for alpha=0 and remap collisions to the nearest opaque colour.
    if let Some(transparent) = frame.transparent {
        let palette = frame
            .palette
            .as_ref()
            .expect("GIF constructor provides local palette");
        for (index, p) in frame
            .buffer
            .to_mut()
            .iter_mut()
            .zip(pixels.as_chunks::<4>().0)
        {
            if p[3] == 0 {
                *index = transparent;
            } else if *index == transparent {
                *index = palette
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != transparent as usize)
                    .min_by_key(|(_, color)| {
                        (0..3)
                            .map(|c| {
                                let d = i32::from(p[c]) - i32::from(color[c]);
                                d * d
                            })
                            .sum::<i32>()
                    })
                    .map(|(i, _)| i as u8)
                    .expect("opaque GIF colour has a palette entry");
            }
        }
    }
    frame.dispose = gif::DisposalMethod::Background;
    frame
}
pub(super) fn gif_still<W: Write>(
    writer: W,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
) -> Result<()> {
    let mut encoder = gif::Encoder::new(writer, width as u16, height as u16, &[])?;
    encoder.write_frame(&gif_frame(width, height, pixels))?;
    encoder.into_inner()?;
    Ok(())
}

pub(super) fn write<W: Write + Seek>(
    writer: &mut W,
    s: &Sprite,
    format: ImageFormat,
    options: &ExportOptions,
) -> Result<()> {
    let timing = timings(s, format)?;
    match format {
        ImageFormat::Gif => {
            let mut encoder = gif::Encoder::new(writer, s.width as u16, s.height as u16, &[])?;
            if options.loop_animation {
                encoder.set_repeat(gif::Repeat::Infinite)?;
            }
            for (i, delay) in timing.ticks.iter().enumerate() {
                let mut frame = gif_frame(s.width, s.height, rgba(s, i, options)?);
                frame.delay = *delay as u16;
                encoder.write_frame(&frame)?;
            }
            encoder.into_inner()?;
        }
        ImageFormat::Apng => {
            let mut encoder = png::Encoder::new(writer, s.width, s.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_animated(
                s.frames.len() as u32,
                if options.loop_animation { 0 } else { 1 },
            )?;
            encoder.set_blend_op(png::BlendOp::Source)?;
            encoder.set_dispose_op(png::DisposeOp::None)?;
            let mut encoder = encoder.write_header()?;
            for (i, delay) in timing.ticks.iter().enumerate() {
                encoder.set_frame_delay(*delay as u16, timing.denominator)?;
                encoder.write_image_data(&rgba(s, i, options)?)?;
            }
            encoder.finish()?;
        }
        ImageFormat::Webp => webp(writer, s, options, &timing)?,
        _ => unreachable!("only animated formats reach animation writer"),
    }
    Ok(())
}

fn u24(writer: &mut impl Write, n: u32) -> Result<()> {
    writer.write_all(&n.to_le_bytes()[..3])?;
    Ok(())
}
fn chunk(writer: &mut impl Write, id: &[u8; 4], data: &[u8]) -> Result<()> {
    writer.write_all(id)?;
    writer.write_all(&(data.len() as u32).to_le_bytes())?;
    writer.write_all(data)?;
    if !data.len().is_multiple_of(2) {
        writer.write_all(&[0])?;
    }
    Ok(())
}
fn webp<W: Write + Seek>(
    writer: &mut W,
    s: &Sprite,
    options: &ExportOptions,
    timing: &Timing,
) -> Result<()> {
    // https://developers.google.com/speed/webp/docs/riff_container
    // VP8X + ANIM + full-canvas ANMF frames. Each ANMF replaces the entire canvas.
    writer.write_all(b"RIFF\0\0\0\0WEBP")?;
    let mut header = Vec::with_capacity(10);
    header.extend_from_slice(&[0x12, 0, 0, 0]); // Alpha + animation flags.
    u24(&mut header, s.width - 1)?;
    u24(&mut header, s.height - 1)?;
    chunk(writer, b"VP8X", &header)?;
    chunk(
        writer,
        b"ANIM",
        &[0, 0, 0, 0, u8::from(!options.loop_animation), 0],
    )?;
    for (i, delay) in timing.ticks.iter().enumerate() {
        let pixels = rgba(s, i, options)?;
        let mut encoded = Cursor::new(Vec::new());
        {
            let bounded = super::BoundedWriter::new(&mut encoded, MAX_EXPORT_BYTES)?;
            image::codecs::webp::WebPEncoder::new_lossless(bounded).write_image(
                &pixels,
                s.width,
                s.height,
                ExtendedColorType::Rgba8,
            )?;
        }
        let encoded = encoded.into_inner();
        ensure!(
            encoded.len() >= 20
                && &encoded[0..4] == b"RIFF"
                && &encoded[8..12] == b"WEBP"
                && &encoded[12..16] == b"VP8L",
            "lossless WebP encoder returned an unsupported container"
        );
        let payload = &encoded[12..];
        let len = 16u32
            .checked_add(payload.len() as u32)
            .ok_or_else(|| anyhow::anyhow!("WebP frame length overflow"))?;
        writer.write_all(b"ANMF")?;
        writer.write_all(&len.to_le_bytes())?;
        u24(writer, 0)?;
        u24(writer, 0)?;
        u24(writer, s.width - 1)?;
        u24(writer, s.height - 1)?;
        u24(writer, *delay)?;
        writer.write_all(&[2])?; // Do not blend; leave canvas until next full replacement.
        writer.write_all(payload)?;
        if len % 2 != 0 {
            writer.write_all(&[0])?;
        }
    }
    let end = writer.stream_position()?;
    ensure!(
        end >= 8 && end - 8 <= u64::from(u32::MAX),
        "WebP RIFF size overflow"
    );
    writer.seek(SeekFrom::Start(4))?;
    writer.write_all(&((end - 8) as u32).to_le_bytes())?;
    writer.seek(SeekFrom::Start(end))?;
    Ok(())
}
