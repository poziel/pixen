//! Exact raster import with bounded decoding and staged output.
use crate::{
    model::Sprite,
    output,
    schema::{self, Annotations, Document, Entry, Frame, Layer},
};
use anyhow::{Context, Result, ensure};
use image::{AnimationDecoder, DynamicImage, ImageDecoder, ImageFormat, RgbaImage};
use std::{
    collections::{BTreeMap, VecDeque},
    io::Cursor,
    path::Path,
};
#[path = "import_apng.rs"]
mod apng;
#[path = "import_webp.rs"]
mod webp;
#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum TransparentMode {
    #[default]
    All,
    Border,
}
#[derive(Default)]
pub struct Options {
    pub transparent_color: Option<[u8; 3]>,
    pub color_tolerance: u8,
    pub transparent_mode: TransparentMode,
    pub trim: bool,
}
pub struct Decoded {
    pub images: Vec<RgbaImage>,
    pub durations: Vec<f64>,
    pub annotations: Annotations,
    pub warnings: Vec<String>,
}
fn limits() -> image::Limits {
    let mut l = image::Limits::default();
    l.max_image_width = Some(4096);
    l.max_image_height = Some(4096);
    l.max_alloc = Some(schema::MAX_IMAGE_BYTES as u64);
    l
}
fn frame_budget(width: u32, height: u32, count: usize) -> Result<()> {
    let bytes = schema::pixel_bytes(width, height)?;
    ensure!(count <= schema::MAX_FRAMES, "animation exceeds 4096 frames");
    ensure!(
        count
            .checked_mul(bytes)
            .is_some_and(|n| n <= schema::MAX_IMAGE_BYTES),
        "animation exceeds total decoded pixel budget (64 Mi pixels)"
    );
    Ok(())
}
// Inspect counts without allocating decoded canvases, then retain the same limits
// while iterating. Chunk lengths are checked before slicing untrusted input.
fn preflight_frames(bytes: &[u8], format: ImageFormat) -> Result<Option<String>> {
    match format {
        ImageFormat::Gif => {
            let mut options = gif::DecodeOptions::new();
            options.skip_frame_decoding(true);
            options.check_frame_consistency(true);
            let mut reader = options.read_info(Cursor::new(bytes))?;
            let (w, h) = (u32::from(reader.width()), u32::from(reader.height()));
            frame_budget(w, h, 0)?;
            let mut count = 0;
            while reader.next_frame_info()?.is_some() {
                count += 1;
                frame_budget(w, h, count)?;
            }
            return Ok(Some(match reader.repeat() {
                gif::Repeat::Infinite => "infinite".into(),
                gif::Repeat::Finite(n) => format!("finite:{}", u32::from(n) + 1),
            }));
        }
        ImageFormat::Png => {
            let mut decoder = png::Decoder::new(Cursor::new(bytes));
            decoder.set_limits(png::Limits {
                bytes: schema::MAX_IMAGE_BYTES,
            });
            let reader = decoder.read_info()?;
            let info = reader.info();
            frame_budget(
                info.width,
                info.height,
                info.animation_control.map_or(1, |a| a.num_frames as usize),
            )?;
        }
        ImageFormat::WebP => {
            ensure!(
                bytes.len() >= 12
                    && u32::from_le_bytes(bytes[4..8].try_into()?) as u64 + 8 == bytes.len() as u64,
                "WebP RIFF size does not match input length"
            );
            let mut position = 12usize;
            let mut count = 0;
            let mut size = None;
            while position < bytes.len() {
                let header = bytes
                    .get(position..position + 8)
                    .context("truncated WebP chunk")?;
                let len = u32::from_le_bytes(header[4..8].try_into()?) as usize;
                let end = position
                    .checked_add(8)
                    .and_then(|p| p.checked_add(len))
                    .context("WebP chunk overflow")?;
                let data = bytes
                    .get(position + 8..end)
                    .context("truncated WebP chunk data")?;
                if &header[..4] == b"VP8X" {
                    ensure!(data.len() == 10, "invalid WebP extended header");
                    let read24 = |b: &[u8]| {
                        u32::from(b[0]) | (u32::from(b[1]) << 8) | (u32::from(b[2]) << 16)
                    };
                    size = Some((read24(&data[4..7]) + 1, read24(&data[7..10]) + 1));
                }
                if &header[..4] == b"ANMF" {
                    count += 1;
                }
                if let Some((w, h)) = size {
                    frame_budget(w, h, count.max(1))?;
                }
                position = end.checked_add(len % 2).context("WebP chunk overflow")?;
            }
        }
        _ => {}
    }
    Ok(None)
}
fn prepare<D: ImageDecoder>(d: &mut D) -> Result<image::metadata::Orientation> {
    let (w, h) = d.dimensions();
    schema::pixel_bytes(w, h)?;
    d.set_limits(limits())?;
    ensure!(
        d.color_type().bits_per_pixel() / d.color_type().channel_count() as u16 <= 8,
        "input exceeds 8 bits per channel; exact RGBA8 representation is impossible"
    );
    Ok(d.orientation()?)
}
fn still<D: ImageDecoder>(mut d: D) -> Result<Decoded> {
    let orientation = prepare(&mut d)?;
    let mut img = DynamicImage::from_decoder(d)?;
    img.apply_orientation(orientation);
    Ok(Decoded {
        images: vec![img.to_rgba8()],
        // Stills have no source timing. Use an integer millisecond duration
        // that all existing native writers can represent without conversion.
        durations: vec![100.0],
        annotations: Annotations::default(),
        warnings: vec![],
    })
}
fn animated<'a, D: AnimationDecoder<'a>>(
    d: D,
    orientation: image::metadata::Orientation,
    dimensions: (u32, u32),
    loop_count: String,
) -> Result<Decoded> {
    schema::pixel_bytes(dimensions.0, dimensions.1)?;
    let mut result = animation_result(loop_count);
    let mut frames = d.into_frames();
    loop {
        let Some(frame) = frames.next() else { break };
        let frame = frame.context("decode animation frame")?;
        frame_budget(dimensions.0, dimensions.1, result.images.len() + 1)?;
        ensure!(
            frame.left() == 0 && frame.top() == 0 && frame.buffer().dimensions() == dimensions,
            "decoder returned an uncomposited animation frame"
        );
        let (n, den) = frame.delay().numer_denom_ms();
        let duration = f64::from(n) / f64::from(den);
        ensure!(
            duration > 0.0,
            "zero-duration animation frame cannot be represented by the schema"
        );
        let mut img = DynamicImage::ImageRgba8(frame.into_buffer());
        img.apply_orientation(orientation);
        result.images.push(img.into_rgba8());
        result.durations.push(duration);
    }
    ensure!(
        !result.images.is_empty(),
        "animation contains no displayed frames"
    );
    Ok(result)
}
fn animation_result(loop_count: String) -> Decoded {
    let mut result = Decoded {
        images: vec![],
        durations: vec![],
        annotations: Annotations::default(),
        warnings: vec![
            format!(
                "animation loop behaviour ({loop_count}) retained as metadata; schema has no playback loop field"
            ),
            "animation delays converted from rational milliseconds to schema f64 milliseconds"
                .into(),
        ],
    };
    result
        .annotations
        .metadata
        .insert("raster_import_loop_count".into(), loop_count.into());
    result
}
pub fn decode(path: &Path) -> Result<Decoded> {
    let bytes = output::read_bounded(path, schema::MAX_JSON)?;
    let format = image::guess_format(&bytes).context("unsupported or malformed raster image")?;
    let source_loops = preflight_frames(&bytes, format)?;
    let reader = Cursor::new(bytes.as_slice());
    match format {
        ImageFormat::Png => {
            let mut d = image::codecs::png::PngDecoder::with_limits(reader, limits())?;
            let orientation = prepare(&mut d)?;
            if d.is_apng()? {
                drop(d);
                apng::decode_apng(&bytes, orientation)
            } else {
                still(d)
            }
        }
        ImageFormat::Jpeg => still(image::codecs::jpeg::JpegDecoder::new(reader)?),
        ImageFormat::Gif => {
            let mut d = image::codecs::gif::GifDecoder::new(reader)?;
            let orientation = prepare(&mut d)?;
            let size = d.dimensions();
            animated(
                d,
                orientation,
                size,
                source_loops.context("missing GIF loop metadata")?,
            )
        }
        ImageFormat::WebP => {
            let mut d = image::codecs::webp::WebPDecoder::new(reader)?;
            let orientation = prepare(&mut d)?;
            if d.has_animation() {
                let size = d.dimensions();
                let loops = match d.loop_count() {
                    image::metadata::LoopCount::Infinite => "infinite".to_owned(),
                    image::metadata::LoopCount::Finite(n) => format!("finite:{n}"),
                };
                drop(d);
                webp::decode_webp(&bytes, size, orientation, loops)
            } else {
                still(d)
            }
        }
        _ => anyhow::bail!("unsupported raster format: use PNG, JPEG, GIF, APNG, or WebP"),
    }
}
fn remove_color(image: &mut RgbaImage, target: [u8; 3], tolerance: u8, mode: TransparentMode) {
    let matches = |p: &image::Rgba<u8>| (0..3).all(|i| p[i].abs_diff(target[i]) <= tolerance);
    if matches!(mode, TransparentMode::All) {
        for p in image.pixels_mut() {
            if matches(p) {
                p[3] = 0;
            }
        }
        return;
    }
    let (w, h) = image.dimensions();
    let mut seen = vec![false; (w * h) as usize];
    let mut queue = VecDeque::new();
    let visit = |x: u32,
                 y: u32,
                 image: &mut RgbaImage,
                 seen: &mut [bool],
                 queue: &mut VecDeque<(u32, u32)>| {
        let i = (y * w + x) as usize;
        if !seen[i] {
            seen[i] = true;
            let p = image.get_pixel_mut(x, y);
            if p[3] == 0 || matches(p) {
                p[3] = 0;
                queue.push_back((x, y));
            }
        }
    };
    for x in 0..w {
        visit(x, 0, image, &mut seen, &mut queue);
        visit(x, h - 1, image, &mut seen, &mut queue);
    }
    for y in 0..h {
        visit(0, y, image, &mut seen, &mut queue);
        visit(w - 1, y, image, &mut seen, &mut queue);
    }
    while let Some((x, y)) = queue.pop_front() {
        if x > 0 {
            visit(x - 1, y, image, &mut seen, &mut queue);
        }
        if x + 1 < w {
            visit(x + 1, y, image, &mut seen, &mut queue);
        }
        if y > 0 {
            visit(x, y - 1, image, &mut seen, &mut queue);
        }
        if y + 1 < h {
            visit(x, y + 1, image, &mut seen, &mut queue);
        }
    }
}
pub fn transform(decoded: &mut Decoded, options: &Options) {
    if let Some(color) = options.transparent_color {
        for img in &mut decoded.images {
            remove_color(
                img,
                color,
                options.color_tolerance,
                options.transparent_mode,
            );
        }
    }
    if !options.trim {
        return;
    }
    let (w, h) = decoded.images[0].dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for img in &decoded.images {
        for (x, y, p) in img.enumerate_pixels() {
            if p[3] > 0 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x0 < x1 && y0 < y1 {
        for img in &mut decoded.images {
            *img = image::imageops::crop_imm(img, x0, y0, x1 - x0, y1 - y0).to_image();
        }
    }
}
pub fn document(decoded: Decoded) -> Result<Document> {
    ensure!(
        !decoded.images.is_empty() && decoded.images.len() == decoded.durations.len(),
        "invalid decoded frame list"
    );
    // Fail at the first unrepresentable colour, before the schema writer builds
    // strings and index grids for the entire image set.
    {
        let mut colours = std::collections::HashSet::new();
        for img in &decoded.images {
            for pixel in img.pixels() {
                colours.insert(pixel.0);
                ensure!(
                    colours.len() <= 1_048_576,
                    "exact palette exceeds 1048576 colours; import never quantizes"
                );
            }
        }
    }
    let (width, height) = decoded.images[0].dimensions();
    let mut images = BTreeMap::new();
    let mut timeline = vec![];
    for (i, img) in decoded.images.into_iter().enumerate() {
        let id = format!("frame_{i:04}");
        images.insert(id.clone(), img.into_raw().into());
        timeline.push(Entry {
            cel: Some(id),
            link: None,
            opacity: 1.0,
            annotations: Annotations::default(),
        });
    }
    let sprite = Sprite {
        width,
        height,
        fps: 12.0,
        annotations: decoded.annotations,
        pixelorama: Default::default(),
        tags: vec![],
        images,
        frames: decoded
            .durations
            .into_iter()
            .map(|duration_ms| Frame {
                duration_ms,
                annotations: Default::default(),
            })
            .collect(),
        layers: vec![Layer {
            id: "artwork".into(),
            name: "Artwork".into(),
            visible: true,
            opacity: 1.0,
            locked: false,
            new_cels_linked: false,
            timeline,
            annotations: Default::default(),
        }],
    };
    let doc = schema::to_document(&sprite)?;
    schema::validate_document(&doc)?;
    Ok(doc)
}
pub fn run(
    input: &Path,
    destination: &Path,
    options: &Options,
    force: bool,
) -> Result<Vec<String>> {
    let name = destination
        .file_name()
        .context("output needs a filename")?
        .to_string_lossy();
    let container = name.ends_with(".pixloom");
    ensure!(
        container || name.ends_with(".pixloom.json"),
        "import output must end in .pixloom.json or .pixloom"
    );
    output::preflight(input, &[destination], force)?;
    if destination.exists() {
        ensure!(
            !same_file(input, destination)?,
            "output aliases input (hard link)"
        );
    }
    let mut decoded = decode(input)?;
    transform(&mut decoded, options);
    let mut warnings = decoded.warnings.clone();
    warnings.push("source colour profiles, comments, text and EXIF are not retained; EXIF orientation is applied to pixels; WebP background hints are ignored in favour of transparency".into());
    let doc = document(decoded)?;
    // Bound serialized size without allocating another in-memory JSON copy.
    output::write_document(std::io::sink(), &doc)?;
    let staged = output::Prepared::new(destination, |file| {
        if container {
            crate::container::write(
                output::LimitedFile::new(file, output::MAX_OUTPUT_BYTES),
                &doc,
            )
        } else {
            output::write_document(file, &doc)
        }
    })?;
    let loaded = if container {
        crate::container::Reader::open(staged.staged_path())?.load()?
    } else {
        schema::read_document(&output::read_bounded(
            staged.staged_path(),
            schema::MAX_JSON,
        )?)?
    };
    ensure!(
        serde_json::to_value(&doc)? == serde_json::to_value(loaded)?,
        "staged import changed document"
    );
    staged.commit(force)?;
    Ok(warnings)
}

#[cfg(unix)]
fn same_file(a: &Path, b: &Path) -> Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let (a, b) = (std::fs::metadata(a)?, std::fs::metadata(b)?);
    Ok(a.dev() == b.dev() && a.ino() == b.ino())
}
#[cfg(windows)]
fn same_file(a: &Path, b: &Path) -> Result<bool> {
    use std::os::windows::io::AsRawHandle;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(handle: *mut std::ffi::c_void, info: *mut [u32; 13]) -> i32;
    }
    let identity = |path: &Path| -> Result<[u32; 3]> {
        let file = std::fs::File::open(path)?;
        // BY_HANDLE_FILE_INFORMATION is thirteen DWORDs. The live File owns
        // the handle and the writable array has the complete required layout.
        let mut info = [0u32; 13];
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok([info[7], info[11], info[12]])
    };
    Ok(identity(a)? == identity(b)?)
}
#[cfg(not(any(unix, windows)))]
fn same_file(a: &Path, b: &Path) -> Result<bool> {
    Ok(a.canonicalize()? == b.canonicalize()?)
}
