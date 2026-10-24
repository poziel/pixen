use super::*;
use image::Rgba;
// Straight-alpha source-over using integer arithmetic. Keep opaque destinations
// opaque; floating point compositing can truncate their alpha to 254.
pub(super) fn over(dst: &mut Rgba<u8>, src: Rgba<u8>) {
    let sa = u32::from(src[3]);
    let da = u32::from(dst[3]);
    if sa == 0 {
        return;
    }
    if sa == 255 || da == 0 {
        *dst = src;
        return;
    }
    let alpha = sa * 255 + da * (255 - sa);
    for i in 0..3 {
        dst[i] = ((u32::from(src[i]) * sa * 255 + u32::from(dst[i]) * da * (255 - sa) + alpha / 2)
            / alpha) as u8;
    }
    dst[3] = ((alpha + 127) / 255) as u8;
}
pub(super) fn decode_apng(
    bytes: &[u8],
    orientation: image::metadata::Orientation,
) -> Result<Decoded> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits {
        bytes: schema::MAX_IMAGE_BYTES,
    });
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info()?;
    let info = reader.info();
    let (w, h) = (info.width, info.height);
    let control = info.animation_control.context("missing APNG control")?;
    frame_budget(w, h, control.num_frames as usize)?;
    let loops = if control.num_plays == 0 {
        "infinite".into()
    } else {
        format!("finite:{}", control.num_plays)
    };
    let mut result = animation_result(loops);
    let mut canvas = RgbaImage::new(w, h);
    let mut buffer = vec![0; reader.output_buffer_size()];
    while result.images.len() < control.num_frames as usize {
        let out = reader.next_frame(&mut buffer)?;
        // A separate default PNG image is a thumbnail, not a displayed frame.
        let Some(fc) = reader.info().frame_control else {
            continue;
        };
        ensure!(
            fc.x_offset.checked_add(fc.width).is_some_and(|x| x <= w)
                && fc.y_offset.checked_add(fc.height).is_some_and(|y| y <= h),
            "APNG frame exceeds canvas"
        );
        let duration = f64::from(fc.delay_num) * 1000.
            / f64::from(if fc.delay_den == 0 { 100 } else { fc.delay_den });
        ensure!(
            duration > 0.,
            "zero-duration animation frame cannot be represented by the schema"
        );
        let previous = if fc.dispose_op == png::DisposeOp::Previous {
            Some(canvas.clone())
        } else {
            None
        };
        let channels = out.color_type.samples();
        for (i, p) in buffer[..out.buffer_size()]
            .chunks_exact(channels)
            .enumerate()
        {
            let rgba = match out.color_type {
                png::ColorType::Rgba => [p[0], p[1], p[2], p[3]],
                png::ColorType::Rgb => [p[0], p[1], p[2], 255],
                png::ColorType::Grayscale => [p[0], p[0], p[0], 255],
                png::ColorType::GrayscaleAlpha => [p[0], p[0], p[0], p[1]],
                _ => anyhow::bail!("unsupported APNG pixel encoding"),
            };
            let x = i as u32 % fc.width + fc.x_offset;
            let y = i as u32 / fc.width + fc.y_offset;
            let dst = canvas.get_pixel_mut(x, y);
            if fc.blend_op == png::BlendOp::Source {
                *dst = Rgba(rgba);
            } else {
                over(dst, Rgba(rgba));
            }
        }
        let mut displayed = DynamicImage::ImageRgba8(canvas.clone());
        displayed.apply_orientation(orientation);
        result.images.push(displayed.into_rgba8());
        result.durations.push(duration);
        match fc.dispose_op {
            png::DisposeOp::Background => {
                for y in fc.y_offset..fc.y_offset + fc.height {
                    for x in fc.x_offset..fc.x_offset + fc.width {
                        canvas.put_pixel(x, y, Rgba([0; 4]));
                    }
                }
            }
            png::DisposeOp::Previous => canvas = previous.unwrap(),
            png::DisposeOp::None => {}
        }
    }
    // Validate the end of the PNG stream too, including trailing CRCs/IEND.
    reader.finish()?;
    Ok(result)
}
