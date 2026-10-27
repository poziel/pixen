//! Desktop export transforms. Source documents are never modified.
use crate::{model::Sprite, schema};
use anyhow::{Context, Result, ensure};

pub fn transform(source: &Sprite, scale: u32, grid: Option<(bool, u32)>) -> Result<Sprite> {
    ensure!((1..=64).contains(&scale), "scale must be between 1 and 64");
    let count = source.frames.len() as u32;
    let (columns, rows) = match grid {
        Some((by_rows, value)) => {
            ensure!(
                value > 0 && value <= count,
                "grid count must be between 1 and the frame count"
            );
            if by_rows {
                (count.div_ceil(value), value)
            } else {
                (value, count.div_ceil(value))
            }
        }
        None => (1, 1),
    };
    let frame_width = source.width.checked_mul(scale).context("width overflow")?;
    let frame_height = source
        .height
        .checked_mul(scale)
        .context("height overflow")?;
    let width = frame_width
        .checked_mul(columns)
        .context("sheet width overflow")?;
    let height = frame_height
        .checked_mul(rows)
        .context("sheet height overflow")?;
    let bytes = schema::pixel_bytes(width, height)?;
    let output_count = if grid.is_some() {
        1
    } else {
        source.frames.len()
    };
    ensure!(
        bytes
            .checked_mul(output_count)
            .is_some_and(|n| n <= 256 * 1024 * 1024),
        "export exceeds 256 MiB pixel budget; reduce scale or grid size"
    );
    let mut images = Vec::new();
    let mut durations = Vec::new();
    if grid.is_some() {
        images.push(image::RgbaImage::new(width, height));
        durations.push(100.0);
    }
    for index in 0..source.frames.len() {
        let pixels = crate::raster::render(source, index)?;
        let image = image::RgbaImage::from_raw(source.width, source.height, pixels)
            .context("invalid pixels")?;
        let scaled = image::imageops::resize(
            &image,
            frame_width,
            frame_height,
            image::imageops::FilterType::Nearest,
        );
        if grid.is_some() {
            image::imageops::replace(
                &mut images[0],
                &scaled,
                i64::from(index as u32 % columns * frame_width),
                i64::from(index as u32 / columns * frame_height),
            );
        } else {
            images.push(scaled);
            durations.push(source.frames[index].duration_ms);
        }
    }
    schema::compile(crate::import::document(crate::import::Decoded {
        images,
        durations,
        annotations: Default::default(),
        warnings: vec![],
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> Sprite {
        schema::compile(
            crate::import::document(crate::import::Decoded {
                images: vec![
                    image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255])),
                    image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 255, 0, 255])),
                    image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 255, 255])),
                ],
                durations: vec![100.0, 200.0, 300.0],
                annotations: Default::default(),
                warnings: vec![],
            })
            .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn grid_preserves_order_and_empty_cells() {
        let sheet = transform(&source(), 2, Some((false, 2))).unwrap();
        assert_eq!((sheet.width, sheet.height, sheet.frames.len()), (4, 4, 1));
        let p = crate::raster::render(&sheet, 0).unwrap();
        assert_eq!(&p[0..4], &[255, 0, 0, 255]);
        assert_eq!(&p[8..12], &[0, 255, 0, 255]);
        assert_eq!(&p[32..36], &[0, 0, 255, 255]);
        assert_eq!(&p[40..44], &[0, 0, 0, 0]);
        let rows = transform(&source(), 1, Some((true, 1))).unwrap();
        assert_eq!((rows.width, rows.height), (3, 1));
    }
    #[test]
    fn scale_preserves_timing_and_rejects_invalid_options() {
        let s = source();
        let scaled = transform(&s, 3, None).unwrap();
        assert_eq!(
            scaled
                .frames
                .iter()
                .map(|f| f.duration_ms)
                .collect::<Vec<_>>(),
            vec![100.0, 200.0, 300.0]
        );
        assert!(transform(&s, 0, None).is_err());
        assert!(transform(&s, 1, Some((false, 0))).is_err());
        assert!(transform(&s, 1, Some((true, 4))).is_err());
    }
}

#[derive(Clone, Copy, Debug, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportKind {
    Frame,
    Sheet,
    Animation,
    Video,
    Project,
}
impl ExportKind {
    fn suffix(self) -> &'static str {
        match self {
            Self::Frame => "frame",
            Self::Sheet => "sheet",
            Self::Animation => "animation",
            Self::Video => "video",
            Self::Project => "project",
        }
    }
}
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportRequest {
    pub format: String,
    pub mode: ExportKind,
    pub frame: usize,
    pub scale: u32,
    pub grid_axis: String,
    pub grid_count: u32,
    pub background: Option<String>,
    pub loop_animation: bool,
}
fn supported(request: &ExportRequest) -> bool {
    let format = request.format.as_str();
    match request.mode {
        ExportKind::Frame | ExportKind::Sheet => matches!(
            format,
            "png" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "tiff" | "apng"
        ),
        ExportKind::Animation => matches!(format, "gif" | "webp" | "apng"),
        ExportKind::Video => matches!(format, "mp4" | "webm"),
        ExportKind::Project => format == "pixen.json",
    }
}
pub fn output_names(name: &str, requests: &[ExportRequest]) -> Result<Vec<String>> {
    ensure!(
        !requests.is_empty() && requests.len() <= 32,
        "choose between 1 and 32 exports"
    );
    ensure!(
        !name.is_empty()
            && name.chars().count() <= 120
            && name.trim() == name
            && !name.ends_with(['.', ' '])
            && !name
                .chars()
                .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c)),
        "invalid export file name"
    );
    let stem = name.split('.').next().unwrap().to_ascii_uppercase();
    ensure!(
        !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            && !(stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9')),
        "reserved export file name"
    );
    let mut used = std::collections::BTreeSet::new();
    let mut names = Vec::new();
    for request in requests {
        ensure!(
            supported(request),
            "{} is not supported for {} export",
            request.format,
            request.mode.suffix()
        );
        let repeated = requests
            .iter()
            .filter(|r| r.format == request.format)
            .count()
            > 1;
        let base = if repeated {
            format!("{name}-{}", request.mode.suffix())
        } else {
            name.to_owned()
        };
        let mut candidate = format!("{base}.{}", request.format);
        let mut suffix = 2;
        while !used.insert(candidate.to_ascii_lowercase()) {
            candidate = format!("{base}-{suffix}.{}", request.format);
            suffix += 1;
        }
        names.push(candidate);
    }
    Ok(names)
}

struct ReadyExport {
    sprite: Option<Sprite>,
    format: Option<crate::formats::OutputFormat>,
    image_options: crate::export::ExportOptions,
    video_options: crate::video::VideoOptions,
}
impl ReadyExport {
    fn prepare(source: &Sprite, request: &ExportRequest) -> Result<Self> {
        ensure!(
            supported(request),
            "format is not supported for this export kind"
        );
        if request.mode == ExportKind::Project {
            return Ok(Self {
                sprite: None,
                format: None,
                image_options: Default::default(),
                video_options: Default::default(),
            });
        }
        let mut sprite = source.clone();
        if request.mode == ExportKind::Frame {
            ensure!(request.frame < sprite.frames.len(), "frame is out of range");
            for layer in &mut sprite.layers {
                layer.timeline = vec![layer.timeline[request.frame].clone()];
            }
            sprite.frames = vec![sprite.frames[request.frame].clone()];
            sprite.tags.clear();
        }
        let background = request
            .background
            .as_deref()
            .map(|hex| -> Result<[u8; 4]> {
                let value = hex.strip_prefix('#').context("background needs #RRGGBB")?;
                ensure!(
                    value.len() == 6 && value.is_ascii(),
                    "background needs #RRGGBB"
                );
                Ok([
                    u8::from_str_radix(&value[0..2], 16)?,
                    u8::from_str_radix(&value[2..4], 16)?,
                    u8::from_str_radix(&value[4..6], 16)?,
                    255,
                ])
            })
            .transpose()?;
        ensure!(
            matches!(request.grid_axis.as_str(), "columns" | "rows"),
            "invalid grid axis"
        );
        sprite = transform(
            &sprite,
            request.scale,
            (request.mode == ExportKind::Sheet)
                .then_some((request.grid_axis == "rows", request.grid_count)),
        )?;
        let format = crate::formats::parse_export(&request.format)?[0];
        let image_options = crate::export::ExportOptions {
            frame: None,
            loop_animation: request.loop_animation
                && request.mode == ExportKind::Animation
                && sprite.frames.len() > 1,
            background,
        };
        let video_options = crate::video::VideoOptions {
            background,
            ..Default::default()
        };
        match format {
            crate::formats::OutputFormat::Image(f) => {
                crate::export::validate(&sprite, f, &image_options)?
            }
            crate::formats::OutputFormat::Video(f) => {
                crate::video::validate(&sprite, f, &video_options)?
            }
        }
        Ok(Self {
            sprite: Some(sprite),
            format: Some(format),
            image_options,
            video_options,
        })
    }
    fn write(&self, file: &mut std::fs::File, document: &schema::Document) -> Result<()> {
        match self.format {
            None => crate::output::write_document(file, document),
            Some(crate::formats::OutputFormat::Image(f)) => {
                crate::export::write(file, self.sprite.as_ref().unwrap(), f, &self.image_options)
            }
            Some(crate::formats::OutputFormat::Video(f)) => {
                crate::video::write(file, self.sprite.as_ref().unwrap(), f, &self.video_options)
            }
        }
    }
}
/// Encode every output into a temporary file before installing any destination.
/// Dropping the returned files cancels the batch without touching existing outputs.
pub fn stage_batch(
    document: &schema::Document,
    requests: &[ExportRequest],
    destinations: &[std::path::PathBuf],
) -> Result<Vec<crate::output::Prepared>> {
    ensure!(
        !requests.is_empty() && requests.len() <= 32 && requests.len() == destinations.len(),
        "invalid export batch"
    );
    let source = schema::compile(document.clone())?;
    let mut prepared = Vec::new();
    for (index, (request, destination)) in requests.iter().zip(destinations).enumerate() {
        let item = ReadyExport::prepare(&source, request)
            .with_context(|| format!("Export {}", index + 1))?;
        prepared.push(
            crate::output::Prepared::new(destination, |file| item.write(file, document))
                .with_context(|| format!("Export {}: {}", index + 1, destination.display()))?,
        );
    }
    Ok(prepared)
}

#[cfg(test)]
mod batch_tests {
    use super::*;
    fn request(mode: ExportKind, format: &str) -> ExportRequest {
        ExportRequest {
            mode,
            format: format.into(),
            frame: 0,
            scale: 1,
            grid_axis: "columns".into(),
            grid_count: 1,
            background: None,
            loop_animation: false,
        }
    }
    #[test]
    fn partial_alpha_animation_exports_all_frames_at_five_times_scale_and_loops() {
        let root = tempfile::tempdir().unwrap();
        let document = crate::import::document(crate::import::Decoded {
            images: (0..5)
                .map(|i| image::RgbaImage::from_pixel(2, 2, image::Rgba([i * 40, 200, 80, 128])))
                .collect(),
            durations: vec![90.0, 45.0, 45.0, 50.0, 25.0],
            annotations: Default::default(),
            warnings: vec![],
        })
        .unwrap();
        let mut export = request(ExportKind::Animation, "gif");
        export.scale = 5;
        export.loop_animation = true;
        let destination = root.path().join("animation.gif");
        for staged in stage_batch(&document, &[export], &[destination.clone()]).unwrap() {
            staged.commit(false).unwrap();
        }
        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);
        let mut decoder = options
            .read_info(std::fs::File::open(destination).unwrap())
            .unwrap();
        assert_eq!((decoder.width(), decoder.height()), (10, 10));
        let mut count = 0;
        while let Some(frame) = decoder.read_next_frame().unwrap() {
            assert!(frame.delay > 0);
            count += 1;
        }
        assert_eq!(count, 5);
        assert_eq!(decoder.repeat(), gif::Repeat::Infinite);
    }
    #[test]
    fn names_distinguish_repeated_formats_and_reject_invalid_kinds_and_stems() {
        let requests = vec![
            request(ExportKind::Frame, "png"),
            request(ExportKind::Sheet, "png"),
            request(ExportKind::Frame, "png"),
            request(ExportKind::Frame, "jpeg"),
        ];
        assert_eq!(
            output_names("hero", &requests).unwrap(),
            vec![
                "hero-frame.png",
                "hero-sheet.png",
                "hero-frame-2.png",
                "hero.jpeg"
            ]
        );
        for name in ["", "../hero", "CON", "nul.txt", "a/b", "hero.", "hero "] {
            assert!(output_names(name, &requests).is_err());
        }
        assert!(output_names("hero", &[request(ExportKind::Video, "png")]).is_err());
        assert!(output_names("hero", &[request(ExportKind::Animation, "jpeg")]).is_err());
        assert!(output_names("hero", &[request(ExportKind::Frame, "pixen.json")]).is_err());
    }
    #[test]
    fn a_failed_batch_does_not_install_or_replace_any_outputs() {
        let root = tempfile::tempdir().unwrap();
        let document = crate::import::document(crate::import::Decoded {
            images: vec![image::RgbaImage::from_pixel(
                2,
                2,
                image::Rgba([255, 0, 0, 128]),
            )],
            durations: vec![100.0],
            annotations: Default::default(),
            warnings: vec![],
        })
        .unwrap();
        let destinations = vec![root.path().join("hero.png"), root.path().join("hero.jpeg")];
        std::fs::write(&destinations[0], b"previous export").unwrap();
        let requests = vec![
            request(ExportKind::Frame, "png"),
            request(ExportKind::Frame, "jpeg"),
        ];
        assert!(stage_batch(&document, &requests, &destinations).is_err());
        assert_eq!(std::fs::read(&destinations[0]).unwrap(), b"previous export");
        assert!(!destinations[1].exists());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }
    #[test]
    fn batch_writes_images_and_editable_json_with_shared_stem() {
        let root = tempfile::tempdir().unwrap();
        let document = crate::import::document(crate::import::Decoded {
            images: vec![image::RgbaImage::from_pixel(
                2,
                2,
                image::Rgba([255, 0, 0, 255]),
            )],
            durations: vec![100.0],
            annotations: Default::default(),
            warnings: vec![],
        })
        .unwrap();
        let requests = vec![
            request(ExportKind::Frame, "png"),
            request(ExportKind::Frame, "jpeg"),
            request(ExportKind::Project, "pixen.json"),
        ];
        let destinations = output_names("hero", &requests)
            .unwrap()
            .iter()
            .map(|n| root.path().join(n))
            .collect::<Vec<_>>();
        let staged = stage_batch(&document, &requests, &destinations).unwrap();
        assert!(destinations.iter().all(|p| !p.exists()));
        for file in staged {
            file.commit(false).unwrap();
        }
        assert_eq!(image::open(&destinations[0]).unwrap().width(), 2);
        assert_eq!(image::open(&destinations[1]).unwrap().height(), 2);
        assert_eq!(crate::input::compile(&destinations[2]).unwrap().width, 2);
    }
}
