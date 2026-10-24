use crate::export::ImageFormat;
use anyhow::{Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFormat {
    Mp4,
    Webm,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectFormat {
    Pxo,
    Aseprite,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Image(ImageFormat),
    Video(VideoFormat),
}
pub const EXPORT_CHOICES: &str =
    "png, svg, jpeg (jpg), gif, webp, bmp, tiff (tif), apng, mp4, webm";
pub const BUILD_CHOICES: &str = "pxo, aseprite (ase)";
pub fn parse_export(input: &str) -> Result<Vec<OutputFormat>> {
    let mut formats = Vec::new();
    for part in input.split(',') {
        let key = part.trim().to_ascii_lowercase();
        let format = match key.as_str() {
            "png" => OutputFormat::Image(ImageFormat::Png),
            "svg" => OutputFormat::Image(ImageFormat::Svg),
            "jpg" | "jpeg" => OutputFormat::Image(ImageFormat::Jpeg),
            "gif" => OutputFormat::Image(ImageFormat::Gif),
            "webp" => OutputFormat::Image(ImageFormat::Webp),
            "bmp" => OutputFormat::Image(ImageFormat::Bmp),
            "tif" | "tiff" => OutputFormat::Image(ImageFormat::Tiff),
            "apng" => OutputFormat::Image(ImageFormat::Apng),
            "mp4" => OutputFormat::Video(VideoFormat::Mp4),
            "webm" => OutputFormat::Video(VideoFormat::Webm),
            _ => bail!("unsupported export format {part:?}; valid choices: {EXPORT_CHOICES}"),
        };
        if !formats.contains(&format) {
            formats.push(format);
        }
    }
    Ok(formats)
}
pub fn parse_build(input: &str) -> Result<Vec<ProjectFormat>> {
    let mut formats = Vec::new();
    for part in input.split(',') {
        let format = match part.trim().to_ascii_lowercase().as_str() {
            "pxo" => ProjectFormat::Pxo,
            "ase" | "aseprite" => ProjectFormat::Aseprite,
            _ => bail!("unsupported build format {part:?}; valid choices: {BUILD_CHOICES}"),
        };
        if !formats.contains(&format) {
            formats.push(format);
        }
    }
    Ok(formats)
}
impl OutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Image(ImageFormat::Png) => "png",
            Self::Image(ImageFormat::Svg) => "svg",
            Self::Image(ImageFormat::Jpeg) => "jpeg",
            Self::Image(ImageFormat::Gif) => "gif",
            Self::Image(ImageFormat::Webp) => "webp",
            Self::Image(ImageFormat::Bmp) => "bmp",
            Self::Image(ImageFormat::Tiff) => "tiff",
            Self::Image(ImageFormat::Apng) => "apng",
            Self::Video(VideoFormat::Mp4) => "mp4",
            Self::Video(VideoFormat::Webm) => "webm",
        }
    }
}
impl ProjectFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Pxo => "pxo",
            Self::Aseprite => "aseprite",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_case_and_duplicates_normalize_in_request_order() {
        assert_eq!(
            parse_export(" PNG, JpG,jpeg,tif,TIFF,gif,webp,apng,mp4,webm,png ")
                .unwrap()
                .iter()
                .map(|f| f.extension())
                .collect::<Vec<_>>(),
            ["png", "jpeg", "tiff", "gif", "webp", "apng", "mp4", "webm"]
        );
        assert_eq!(
            parse_build(" PXO,ase,ASEPRITE,pxo").unwrap(),
            [ProjectFormat::Pxo, ProjectFormat::Aseprite]
        );
    }
    #[test]
    fn invalid_formats_are_actionable_and_categories_are_separate() {
        for request in ["", "png,", "unknown", "pxo"] {
            assert!(
                parse_export(request)
                    .unwrap_err()
                    .to_string()
                    .contains("png")
            );
        }
        for request in ["", "pxo,", "png", "unknown"] {
            assert!(
                parse_build(request)
                    .unwrap_err()
                    .to_string()
                    .contains("aseprite")
            );
        }
    }
}
