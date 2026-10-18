use crate::model::Sprite;
use anyhow::{Result, ensure};
pub fn render(sprite: &Sprite, frame: usize) -> Result<Vec<u8>> {
    ensure!(
        frame < sprite.frames.len(),
        "frame {} is out of range (1..{})",
        frame + 1,
        sprite.frames.len()
    );
    let len = crate::schema::pixel_bytes(sprite.width, sprite.height)?;
    let mut acc = vec![[0.0f64; 4]; len / 4];
    for layer in &sprite.layers {
        if !layer.visible {
            continue;
        }
        let cel = &layer.timeline[frame];
        let Some(id) = &cel.cel else { continue };
        let opacity = layer.opacity * cel.opacity;
        for (d, p) in acc.iter_mut().zip(sprite.images[id].as_chunks::<4>().0) {
            let a = p[3] as f64 / 255.0 * opacity;
            for c in 0..3 {
                d[c] = p[c] as f64 * a + d[c] * (1.0 - a);
            }
            d[3] = a + d[3] * (1.0 - a);
        }
    }
    let mut out = Vec::with_capacity(len);
    for p in acc {
        if p[3] == 0.0 {
            out.extend_from_slice(&[0; 4]);
            continue;
        }
        for c in &p[..3] {
            out.push((c / p[3]).round().clamp(0.0, 255.0) as u8);
        }
        out.push((p[3] * 255.0).round().clamp(0.0, 255.0) as u8);
    }
    Ok(out)
}
pub fn png<W: std::io::Write>(writer: W, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    ensure!(
        rgba.len() as u64 == u64::from(width) * u64::from(height) * 4,
        "PNG byte count mismatch"
    );
    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;
    writer.finish()?;
    Ok(())
}

/// Convert rendered RGBA to RGB, requiring an opaque matte for transparent pixels.
pub fn opaque_rgb(rgba: &[u8], background: Option<[u8; 4]>) -> Result<Vec<u8>> {
    ensure!(rgba.len().is_multiple_of(4), "invalid RGBA buffer length");
    if let Some(bg) = background {
        ensure!(bg[3] == 255, "background colour must be opaque (#RRGGBBFF)");
    }
    ensure!(
        background.is_some() || rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255),
        "transparent content requires an explicit opaque --background #RRGGBBAA for this format"
    );
    let bg = background.unwrap_or([0, 0, 0, 255]);
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    for p in rgba.as_chunks::<4>().0 {
        let a = u32::from(p[3]);
        for c in 0..3 {
            rgb.push(((u32::from(p[c]) * a + u32::from(bg[c]) * (255 - a) + 127) / 255) as u8);
        }
    }
    Ok(rgb)
}
pub fn contact_sheet(s: &Sprite, columns: u32) -> Result<(u32, u32, Vec<u8>)> {
    ensure!(
        columns > 0 && columns as usize <= s.frames.len(),
        "columns must be 1..frame count"
    );
    let rows = (s.frames.len() as u32).div_ceil(columns);
    let width = s
        .width
        .checked_mul(columns)
        .ok_or_else(|| anyhow::anyhow!("sheet width overflow"))?;
    let height = s
        .height
        .checked_mul(rows)
        .ok_or_else(|| anyhow::anyhow!("sheet height overflow"))?;
    ensure!(
        u64::from(width) * u64::from(height) <= 64 * 1024 * 1024,
        "contact sheet exceeds 64 million pixels"
    );
    let mut out = vec![0; (u64::from(width) * u64::from(height) * 4) as usize];
    for frame in 0..s.frames.len() {
        let rgba = render(s, frame)?;
        let x = (frame as u32 % columns) * s.width;
        let y = (frame as u32 / columns) * s.height;
        for row in 0..s.height {
            let dst = ((y + row) as usize * width as usize + x as usize) * 4;
            let src = row as usize * s.width as usize * 4;
            out[dst..dst + s.width as usize * 4]
                .copy_from_slice(&rgba[src..src + s.width as usize * 4]);
        }
    }
    Ok((width, height, out))
}

#[cfg(test)]
mod tests {
    #[test]
    fn opaque_formats_require_background_and_composite_exactly() {
        let pixels = [255, 0, 0, 128, 10, 20, 30, 255];
        assert!(super::opaque_rgb(&pixels, None).is_err());
        assert_eq!(
            super::opaque_rgb(&pixels, Some([255; 4])).unwrap(),
            [255, 127, 127, 10, 20, 30]
        );
        assert_eq!(super::opaque_rgb(&pixels[4..], None).unwrap(), [10, 20, 30]);
        assert!(super::opaque_rgb(&pixels, Some([0, 0, 0, 128])).is_err());
    }
    #[test]
    fn contact_sheet_orders_frames_and_pads_with_transparency() {
        let sprite =
            crate::schema::parse(include_bytes!("../skills/pixloom/examples/animated.json"))
                .unwrap();
        let (w, h, rgba) = super::contact_sheet(&sprite, 3).unwrap();
        assert_eq!((w, h), (12, 8));
        for (x, color) in [
            (1, [244, 201, 93, 255]),
            (5, [250, 240, 202, 255]),
            (9, [244, 201, 93, 255]),
        ] {
            let p = (12 + x) * 4;
            assert_eq!(&rgba[p..p + 4], &color);
        }
        assert!(rgba[12 * 4 * 4..].iter().all(|b| *b == 0));
        assert!(super::contact_sheet(&sprite, 0).is_err());
        assert!(super::contact_sheet(&sprite, 5).is_err());
    }
}
