//! Read-only view conversions; never change the source document.
use crate::{model::Sprite, raster, schema};
use anyhow::{Context, Result, ensure};
use image::RgbaImage;

fn sprite(images: Vec<RgbaImage>, durations: Vec<f64>) -> Result<Sprite> {
    schema::compile(crate::import::document(crate::import::Decoded {
        images,
        durations,
        annotations: Default::default(),
        warnings: vec![],
    })?)
}

pub fn sheet(source: &Sprite) -> Result<Sprite> {
    let count = source.frames.len() as u32;
    ensure!(count > 1, "no animation frames");
    let columns = (count as f64).sqrt().ceil() as u32;
    let rows = count.div_ceil(columns);
    let width = source
        .width
        .checked_mul(columns)
        .context("sheet width overflow")?;
    let height = source
        .height
        .checked_mul(rows)
        .context("sheet height overflow")?;
    schema::pixel_bytes(width, height)?;
    let mut image = RgbaImage::new(width, height);
    for frame in 0..count {
        let pixels = RgbaImage::from_raw(
            source.width,
            source.height,
            raster::render(source, frame as usize)?,
        )
        .context("invalid frame pixels")?;
        image::imageops::replace(
            &mut image,
            &pixels,
            i64::from(frame % columns * source.width),
            i64::from(frame / columns * source.height),
        );
    }
    sprite(vec![image], vec![100.0])
}

/// Conservative recognition of square strips/grids with uniform gutters.
/// An arbitrary image's dimensions are not evidence of animation.
pub fn animation(source: &Sprite) -> Result<Option<Sprite>> {
    if source.frames.len() != 1 {
        return Ok(None);
    }
    let image = RgbaImage::from_raw(source.width, source.height, raster::render(source, 0)?)
        .context("invalid image pixels")?;
    let (width, height) = image.dimensions();
    let background = image.get_pixel(0, 0).0;
    let empty = |p: &image::Rgba<u8>| {
        if background[3] <= 8 {
            p[3] <= 8
        } else {
            p[3] >= 247 && (0..3).all(|c| p[c].abs_diff(background[c]) <= 12)
        }
    };
    let mut sizes = vec![width.min(height)];
    sizes.extend([8, 16, 24, 32, 48, 64, 96, 128, 192, 256]);
    sizes.sort_unstable();
    sizes.dedup();
    for size in sizes {
        if size < 4 || width % size != 0 || height % size != 0 {
            continue;
        }
        let columns = width / size;
        let rows = height / size;
        let count = columns * rows;
        if !(2..=256).contains(&count) {
            continue;
        }
        // Every internal boundary must include a blank separator line.
        let vertical = (1..columns).all(|col| {
            [col * size - 1, col * size]
                .into_iter()
                .any(|x| (0..height).all(|y| empty(image.get_pixel(x, y))))
        });
        let horizontal = (1..rows).all(|row| {
            [row * size - 1, row * size]
                .into_iter()
                .any(|y| (0..width).all(|x| empty(image.get_pixel(x, y))))
        });
        if !vertical || !horizontal {
            continue;
        }
        let mut frames = Vec::new();
        let mut valid = true;
        for row in 0..rows {
            for col in 0..columns {
                let frame = image::imageops::crop_imm(&image, col * size, row * size, size, size)
                    .to_image();
                let occupied = frame.pixels().filter(|p| !empty(p)).count();
                if occupied == 0 || occupied as f64 > f64::from(size * size) * 0.9 {
                    valid = false;
                    break;
                }
                frames.push(frame);
            }
        }
        if !valid || frames.iter().all(|f| f == &frames[0]) {
            continue;
        }
        let durations = vec![100.0; frames.len()];
        return sprite(frames, durations).map(Some);
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_strips_and_grids_but_rejects_plain_images() {
        for (cols, rows) in [(4, 1), (1, 4), (2, 2)] {
            let mut image = RgbaImage::new(cols * 16, rows * 16);
            for row in 0..rows {
                for col in 0..cols {
                    for y in 3..13 {
                        for x in 3..13 {
                            image.put_pixel(
                                col * 16 + x,
                                row * 16 + y,
                                image::Rgba([(row * cols + col) as u8 * 40, 100, 200, 255]),
                            );
                        }
                    }
                }
            }
            let source = sprite(vec![image], vec![100.0]).unwrap();
            let frames = animation(&source).unwrap().unwrap();
            assert_eq!(frames.frames.len(), (cols * rows) as usize);
            assert_eq!((frames.width, frames.height), (16, 16));
            let sheet = sheet(&frames).unwrap();
            assert_eq!(sheet.frames.len(), 1);
            assert_eq!((sheet.width, sheet.height), (32, 32));
        }
        let plain = sprite(
            vec![RgbaImage::from_pixel(
                64,
                16,
                image::Rgba([20, 40, 60, 255]),
            )],
            vec![100.0],
        )
        .unwrap();
        assert!(animation(&plain).unwrap().is_none());
    }
}
