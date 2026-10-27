//! Snapshot user-selected references; only images and readable UTF-8 enter model input.
use super::*;

#[derive(Clone)]
pub struct Attachment {
    pub name: String,
    pub path: PathBuf,
    pub text: Option<String>,
    pub preview: Option<image::RgbaImage>,
    _directory: Arc<tempfile::TempDir>,
}

impl Attachment {
    pub fn load(path: &std::path::Path) -> Result<Self> {
        ensure!(
            std::fs::metadata(path)?.len() <= 16 * 1024 * 1024,
            "Reference file exceeds 16 MiB"
        );
        let bytes = std::fs::read(path)?;
        let directory = Arc::new(crate::storage::temporary_dir("pixen-reference-")?);
        let name = path
            .file_name()
            .context("Reference needs a filename")?
            .to_string_lossy()
            .into_owned();
        let mut attachment = Self {
            name,
            path: directory.path().join("reference.txt"),
            text: None,
            preview: None,
            _directory: directory,
        };
        let mut reader =
            image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format()?;
        if reader.format().is_some() {
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(128 * 1024 * 1024);
            limits.max_image_width = Some(8192);
            limits.max_image_height = Some(8192);
            reader.limits(limits);
            let image = reader
                .decode()
                .context("Cannot decode reference image")?
                .to_rgba8();
            attachment.path = attachment._directory.path().join("reference.png");
            image.save_with_format(&attachment.path, image::ImageFormat::Png)?;
            let preview = image::imageops::thumbnail(&image, 96, 64);
            attachment.preview = Some(preview);
        } else {
            ensure!(bytes.len() <= 1024 * 1024, "Text reference exceeds 1 MiB");
            let text = String::from_utf8(bytes)
                .context("Unsupported file: attach an image or UTF-8 text document")?;
            ensure!(
                !text
                    .chars()
                    .any(|c| c.is_control() && !['\n', '\r', '\t'].contains(&c)),
                "Unsupported binary reference; attach an image or UTF-8 text document"
            );
            std::fs::write(&attachment.path, text.as_bytes())?;
            attachment.text = Some(text);
        }
        Ok(attachment)
    }
    pub fn input(&self) -> Value {
        if let Some(text) = &self.text {
            json!({"type":"text","text":format!("[Reference document: {}]\n{}\n[End reference document]",self.name,text)})
        } else {
            json!({"type":"localImage","path":self.path})
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_and_text_inputs_are_snapshots_not_mutable_originals() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("brief.txt");
        std::fs::write(&text, "Use a green palette").unwrap();
        let reference = Attachment::load(&text).unwrap();
        std::fs::write(&text, "Changed").unwrap();
        assert!(
            reference.input()["text"]
                .as_str()
                .unwrap()
                .contains("Use a green palette")
        );
        let png = dir.path().join("ref.png");
        image::RgbaImage::from_pixel(4, 2, image::Rgba([255, 0, 0, 255]))
            .save(&png)
            .unwrap();
        let reference = Attachment::load(&png).unwrap();
        std::fs::remove_file(png).unwrap();
        assert!(reference.path.is_file());
        assert_eq!(reference.input()["type"], "localImage");
        assert!(reference.preview.is_some());
        let binary = dir.path().join("unknown.bin");
        std::fs::write(&binary, [0, 255, 12]).unwrap();
        assert!(Attachment::load(&binary).is_err());
    }
}
