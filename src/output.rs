use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub const MAX_OUTPUT_BYTES: u64 = 512 * 1024 * 1024;

/// Append format extensions to an explicitly extensionless directory/stem base.
pub fn base_paths(base: &Path, extensions: &[&str]) -> Result<Vec<PathBuf>> {
    ensure!(
        !extensions.is_empty(),
        "at least one output format is required"
    );
    let name = base
        .file_name()
        .context("--output needs a directory and filename stem")?;
    ensure!(
        base.extension().is_none(),
        "--output must be an extensionless base path, e.g. assets/heart"
    );
    let text = base.as_os_str().to_string_lossy();
    ensure!(
        !text.ends_with(['/', '\\']),
        "--output must end with a filename stem, not a directory separator"
    );
    let parent = base.parent().unwrap_or(Path::new(""));
    extensions
        .iter()
        .map(|extension| {
            ensure!(
                !extension.is_empty() && extension.bytes().all(|b| b.is_ascii_alphanumeric()),
                "invalid format extension"
            );
            let mut file = name.to_os_string();
            file.push(".");
            file.push(extension);
            Ok(parent.join(file))
        })
        .collect()
}

/// Bound seekable encoders, including TIFF and native archive header backpatches.
pub struct LimitedFile<'a> {
    file: &'a mut fs::File,
    limit: u64,
}
impl<'a> LimitedFile<'a> {
    pub fn new(file: &'a mut fs::File, limit: u64) -> Self {
        Self { file, limit }
    }
}
impl Write for LimitedFile<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let end = self
            .file
            .stream_position()?
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| std::io::Error::other("output position overflow"))?;
        if end > self.limit {
            return Err(std::io::Error::other("output exceeds size limit"));
        }
        self.file.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
impl Seek for LimitedFile<'_> {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        let target = match from {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(n) => i128::from(self.file.stream_position()?) + i128::from(n),
            SeekFrom::End(n) => i128::from(self.file.metadata()?.len()) + i128::from(n),
        };
        if target < 0 || target > i128::from(self.limit) {
            return Err(std::io::Error::other("seek exceeds output size limit"));
        }
        self.file.seek(SeekFrom::Start(target as u64))
    }
}

pub fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    ensure!(file.metadata()?.is_file(), "input must be a regular file");
    ensure!(
        file.metadata()?.len() <= limit,
        "input exceeds {limit} bytes"
    );
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "input exceeds {limit} bytes");
    Ok(bytes)
}
pub fn resolved(path: &Path) -> Result<PathBuf> {
    let name = path.file_name().context("output needs a filename")?;
    #[cfg(windows)]
    {
        let name = name.to_string_lossy();
        ensure!(
            !name.ends_with(['.', ' ']) && !name.contains(':'),
            "ambiguous Windows output filename: {}",
            path.display()
        );
    }
    if path.exists() {
        return Ok(path.canonicalize()?);
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Ok(parent
        .canonicalize()
        .with_context(|| format!("output parent {} must exist", parent.display()))?
        .join(name))
}
pub fn preflight(input: &Path, outputs: &[&Path], force: bool) -> Result<()> {
    let input = input.canonicalize()?;
    let mut seen = std::collections::HashSet::new();
    for &p in outputs {
        let abs = resolved(p)?;
        ensure!(abs != input, "output aliases input: {}", p.display());
        #[cfg(windows)]
        let key = abs.to_string_lossy().to_lowercase();
        #[cfg(not(windows))]
        let key = abs;
        ensure!(seen.insert(key), "outputs refer to the same file");
        if let Ok(meta) = fs::symlink_metadata(p) {
            ensure!(
                meta.is_file() && !meta.file_type().is_symlink(),
                "output must be a regular file: {}",
                p.display()
            );
            ensure!(
                force,
                "output already exists: {} (use --force)",
                p.display()
            );
        }
    }
    Ok(())
}
pub struct Prepared {
    temp: tempfile::NamedTempFile,
    path: PathBuf,
}
impl Prepared {
    pub fn new(path: &Path, write: impl FnOnce(&mut fs::File) -> Result<()>) -> Result<Self> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        write(temp.as_file_mut())?;
        temp.as_file().sync_all()?;
        Ok(Self {
            temp,
            path: path.to_owned(),
        })
    }
    pub fn staged_path(&self) -> &Path {
        self.temp.path()
    }
    pub fn commit(self, force: bool) -> Result<()> {
        if force {
            self.temp.persist(&self.path).map_err(|e| e.error)?;
        } else {
            self.temp
                .persist_noclobber(&self.path)
                .map_err(|e| e.error)?;
        }
        Ok(())
    }
}
pub struct BoundedWriter<W> {
    inner: W,
    remaining: u64,
}

pub fn write_document<W: Write>(writer: W, doc: &crate::schema::Document) -> Result<()> {
    crate::schema::validate_document(doc)?;
    let mut writer = std::io::BufWriter::new(BoundedWriter::new(writer, crate::schema::MAX_JSON));
    serde_json::to_writer(&mut writer, doc)?;
    writer.flush()?;
    Ok(())
}

impl<W> BoundedWriter<W> {
    pub fn new(inner: W, limit: u64) -> Self {
        Self {
            inner,
            remaining: limit,
        }
    }
}
impl<W: Write> Write for BoundedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() as u64 > self.remaining {
            return Err(std::io::Error::other("output exceeds size limit"));
        }
        let count = self.inner.write(bytes)?;
        self.remaining -= count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn base_paths_append_extensions_and_reject_filenames_and_directories() {
        use std::path::Path;
        assert_eq!(
            super::base_paths(Path::new("assets.v1/heart"), &["png", "svg"]).unwrap(),
            [
                Path::new("assets.v1/heart.png"),
                Path::new("assets.v1/heart.svg")
            ]
        );
        for path in ["assets/heart.png", "assets/heart/", "/"] {
            assert!(super::base_paths(Path::new(path), &["png"]).is_err());
        }
    }
    #[test]
    fn encoder_writes_and_header_seeks_cannot_exceed_the_output_budget() {
        use std::io::{Seek, SeekFrom, Write};
        let mut temp = tempfile::tempfile().unwrap();
        let mut limited = super::LimitedFile::new(&mut temp, 8);
        limited.write_all(&[1; 8]).unwrap();
        assert!(limited.write_all(&[1]).is_err());
        assert!(limited.seek(SeekFrom::Start(9)).is_err());
        assert!(limited.seek(SeekFrom::Current(-9)).is_err());
        limited.seek(SeekFrom::Start(0)).unwrap();
        limited.write_all(&[2; 4]).unwrap();
    }
    #[derive(Default)]
    struct CountingWriter {
        calls: usize,
        bytes: Vec<u8>,
    }
    impl std::io::Write for CountingWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.calls += 1;
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn authoring_export_buffers_small_writes_and_stays_compact() {
        let sprite =
            crate::schema::parse(include_bytes!("../skills/pixloom/examples/animated.json"))
                .unwrap();
        let doc = crate::schema::to_document(&sprite).unwrap();
        let mut writer = CountingWriter::default();
        super::write_document(&mut writer, &doc).unwrap();
        assert!(writer.calls <= 2, "{} tiny filesystem writes", writer.calls);
        assert!(writer.bytes.len() < 2000, "expanded JSON whitespace");
        crate::schema::parse(&writer.bytes).unwrap();
    }
}
