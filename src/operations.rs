//! Reusable multi-output operations. JSON/model construction occurs once per request.
use crate::{
    export::ExportOptions,
    formats::{OutputFormat, ProjectFormat},
    output::{self, LimitedFile, Prepared},
    video::VideoOptions,
};
use anyhow::{Result, ensure};
use std::path::{Path, PathBuf};

pub fn export(
    input: &Path,
    base: &Path,
    formats: &[OutputFormat],
    images: &ExportOptions,
    videos: &VideoOptions,
    force: bool,
) -> Result<Vec<PathBuf>> {
    let extensions: Vec<_> = formats.iter().map(|f| f.extension()).collect();
    let paths = output::base_paths(base, &extensions)?;
    output::preflight(
        input,
        &paths.iter().map(PathBuf::as_path).collect::<Vec<_>>(),
        force,
    )?;
    ensure!(
        images.frame.is_none() || !formats.iter().any(|f| matches!(f, OutputFormat::Video(_))),
        "--frame cannot be used when video output is requested; video exports the full animation"
    );
    let sprite = crate::input::compile(input)?;
    for format in formats {
        match format {
            OutputFormat::Image(f) => crate::export::validate(&sprite, *f, images)?,
            OutputFormat::Video(f) => crate::video::validate(&sprite, *f, videos)?,
        }
    }
    let mut prepared = Vec::with_capacity(paths.len());
    for (format, path) in formats.iter().zip(&paths) {
        prepared.push(Prepared::new(path, |file| {
            match format {
                OutputFormat::Image(f) => crate::export::write(
                    LimitedFile::new(file, output::MAX_OUTPUT_BYTES),
                    &sprite,
                    *f,
                    images,
                )?,
                OutputFormat::Video(f) => crate::video::write(file, &sprite, *f, videos)?,
            }
            ensure!(
                file.metadata()?.len() <= output::MAX_OUTPUT_BYTES,
                "encoded output exceeds 512 MiB"
            );
            Ok(())
        })?);
    }
    for file in prepared {
        file.commit(force)?;
    }
    Ok(paths)
}

pub fn build(
    input: &Path,
    base: &Path,
    formats: &[ProjectFormat],
    force: bool,
) -> Result<Vec<PathBuf>> {
    let extensions: Vec<_> = formats.iter().map(|f| f.extension()).collect();
    let paths = output::base_paths(base, &extensions)?;
    output::preflight(
        input,
        &paths.iter().map(PathBuf::as_path).collect::<Vec<_>>(),
        force,
    )?;
    let sprite = crate::input::compile(input)?;
    let mut prepared = Vec::with_capacity(paths.len());
    for (format, path) in formats.iter().zip(&paths) {
        prepared.push(Prepared::new(path, |file| {
            crate::native::write(
                LimitedFile::new(file, output::MAX_OUTPUT_BYTES),
                &sprite,
                *format,
            )?;
            ensure!(
                file.metadata()?.len() <= output::MAX_OUTPUT_BYTES,
                "native project exceeds 512 MiB"
            );
            Ok(())
        })?);
    }
    for file in prepared {
        file.commit(force)?;
    }
    Ok(paths)
}
