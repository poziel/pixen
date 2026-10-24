//! Shared native-project dispatch. A recognized signature must agree with its extension.
use crate::{formats::ProjectFormat, model::Sprite};
use anyhow::{Context, Result, ensure};
use std::{
    fs::File,
    io::{Read, Seek, Write},
    path::Path,
};

pub fn detect(path: &Path) -> Result<ProjectFormat> {
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    ensure!(
        file.metadata()?.is_file(),
        "native input must be a regular file"
    );
    ensure!(
        file.metadata()?.len() <= crate::archive::MAX_ARCHIVE,
        "native input exceeds 512 MiB"
    );
    let mut header = [0; 8];
    let count = file.read(&mut header)?;
    let format = if count >= 4 && [&b"PK\x03\x04"[..], &b"PK\x05\x06"[..]].contains(&&header[..4]) {
        ProjectFormat::Pxo
    } else if count >= 6 && header[4..6] == [0xe0, 0xa5] {
        ProjectFormat::Aseprite
    } else {
        anyhow::bail!(
            "unrecognized native file signature; expected Pixelorama PXO ZIP or Aseprite RGBA project"
        )
    };
    if let Some(extension) = path.extension() {
        let extension = extension
            .to_str()
            .context("native extension is not UTF-8")?
            .to_ascii_lowercase();
        let expected = match extension.as_str() {
            "pxo" => Some(ProjectFormat::Pxo),
            "ase" | "aseprite" => Some(ProjectFormat::Aseprite),
            _ => None,
        };
        ensure!(
            expected == Some(format),
            "extension/signature mismatch: .{extension} file has {} signature; use .{}",
            format.extension(),
            format.extension()
        );
    }
    Ok(format)
}
pub fn read(path: &Path) -> Result<Sprite> {
    match detect(path)? {
        ProjectFormat::Pxo => crate::pxo::read(path),
        ProjectFormat::Aseprite => crate::aseprite::read(path),
    }
}
pub fn inspect(path: &Path) -> Result<serde_json::Value> {
    let format = detect(path)?;
    let mut info = match format {
        ProjectFormat::Pxo => crate::pxo::inspect(path)?,
        ProjectFormat::Aseprite => crate::aseprite::inspect(path)?,
    };
    info["format"] = serde_json::json!(format.extension());
    Ok(info)
}
pub fn write<W: Write + Seek>(writer: W, s: &Sprite, format: ProjectFormat) -> Result<()> {
    match format {
        ProjectFormat::Pxo => crate::pxo::write(writer, s),
        ProjectFormat::Aseprite => crate::aseprite::write(writer, s),
    }
}
