use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

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
            crate::schema::parse(include_bytes!("../skills/spritec/examples/animated.json"))
                .unwrap();
        let doc = crate::schema::to_document(&sprite).unwrap();
        let mut writer = CountingWriter::default();
        super::write_document(&mut writer, &doc).unwrap();
        assert!(writer.calls <= 2, "{} tiny filesystem writes", writer.calls);
        assert!(writer.bytes.len() < 2000, "expanded JSON whitespace");
        crate::schema::parse(&writer.bytes).unwrap();
    }
}
