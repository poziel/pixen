//! Decode individual WebP bitstreams with the existing codec, composite in RGBA.
use super::*;
use image::Rgba;
fn u24(bytes: &[u8]) -> u32 {
    u32::from(bytes[0]) | (u32::from(bytes[1]) << 8) | (u32::from(bytes[2]) << 16)
}
pub(super) fn decode_webp(
    bytes: &[u8],
    size: (u32, u32),
    orientation: image::metadata::Orientation,
    loops: String,
) -> Result<Decoded> {
    let mut result = animation_result(loops);
    let mut canvas = RgbaImage::new(size.0, size.1);
    let mut pos = 12;
    while pos < bytes.len() {
        let header = bytes.get(pos..pos + 8).context("truncated WebP chunk")?;
        let len = u32::from_le_bytes(header[4..8].try_into()?) as usize;
        let end = pos
            .checked_add(8)
            .and_then(|n| n.checked_add(len))
            .context("WebP chunk overflow")?;
        let data = bytes.get(pos + 8..end).context("truncated WebP chunk")?;
        pos = end + len % 2;
        if &header[..4] != b"ANMF" {
            continue;
        }
        ensure!(data.len() >= 16, "truncated WebP frame header");
        let (x, y, w, h) = (
            u24(&data[0..3]) * 2,
            u24(&data[3..6]) * 2,
            u24(&data[6..9]) + 1,
            u24(&data[9..12]) + 1,
        );
        frame_budget(size.0, size.1, result.images.len() + 1)?;
        schema::pixel_bytes(w, h)?;
        ensure!(
            x.checked_add(w).is_some_and(|n| n <= size.0)
                && y.checked_add(h).is_some_and(|n| n <= size.1),
            "WebP frame exceeds canvas"
        );
        ensure!(data[15] & !3 == 0, "unsupported WebP frame flags");
        let duration = u24(&data[12..15]);
        ensure!(
            duration > 0,
            "zero-duration animation frame cannot be represented by the schema"
        );
        // Frame payloads may contain ALPH+VP8 or VP8L. A static extended
        // wrapper makes both forms available through the existing decoder.
        let mut wrapped = b"RIFF".to_vec();
        wrapped.extend(((data.len() - 16 + 22) as u32).to_le_bytes());
        wrapped.extend(b"WEBPVP8X\x0a\0\0\0\x10\0\0\0");
        wrapped.extend(&(w - 1).to_le_bytes()[..3]);
        wrapped.extend(&(h - 1).to_le_bytes()[..3]);
        wrapped.extend(&data[16..]);
        let frame = still(image::codecs::webp::WebPDecoder::new(Cursor::new(wrapped))?)?
            .images
            .remove(0);
        ensure!(
            frame.dimensions() == (w, h),
            "WebP frame dimensions disagree with bitstream"
        );
        for (fx, fy, p) in frame.enumerate_pixels() {
            let dst = canvas.get_pixel_mut(x + fx, y + fy);
            if data[15] & 2 != 0 {
                *dst = *p;
            } else {
                super::apng::over(dst, *p);
            }
        }
        let mut displayed = DynamicImage::ImageRgba8(canvas.clone());
        displayed.apply_orientation(orientation);
        result.images.push(displayed.into_rgba8());
        result.durations.push(f64::from(duration));
        if data[15] & 1 != 0 {
            for cy in y..y + h {
                for cx in x..x + w {
                    canvas.put_pixel(cx, cy, Rgba([0; 4]));
                }
            }
        }
    }
    ensure!(
        !result.images.is_empty(),
        "animation contains no displayed frames"
    );
    Ok(result)
}
