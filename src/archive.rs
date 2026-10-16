use anyhow::{Context, Result, ensure};
use std::{
    collections::HashSet,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
pub const MAX_ARCHIVE: u64 = 512 * 1024 * 1024;
pub const MAX_ENTRY: u64 = 64 * 1024 * 1024;
pub const MAX_EXPANDED: u64 = 8 * 1024 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 65534;

// zip 2.x indexes names in a map and hides duplicate central-directory entries.
// Check the bounded classic ZIP directory before it allocates or normalizes names.
fn check_directory(file: &mut File) -> Result<usize> {
    let size = file.metadata()?.len();
    let tail_len = size.min(65557) as usize;
    file.seek(SeekFrom::End(-(tail_len as i64)))?;
    let mut tail = vec![0; tail_len];
    file.read_exact(&mut tail)?;
    let pos = (0..tail_len.saturating_sub(21))
        .rev()
        .find(|&p| {
            tail[p..p + 4] == *b"PK\x05\x06"
                && p + 22 + u16::from_le_bytes([tail[p + 20], tail[p + 21]]) as usize == tail_len
        })
        .context("missing classic ZIP end record")?;
    let end = size - tail_len as u64 + pos as u64;
    let e = &tail[pos..pos + 22];
    let u16_at = |p: usize| u16::from_le_bytes([e[p], e[p + 1]]);
    let u32_at = |p: usize| u32::from_le_bytes(e[p..p + 4].try_into().unwrap());
    ensure!(
        u16_at(4) == 0 && u16_at(6) == 0 && u16_at(8) == u16_at(10),
        "multi-disk ZIP is unsupported"
    );
    let count = u16_at(10) as usize;
    ensure!(
        count <= MAX_ENTRIES,
        "ZIP64 or excessive entry count is unsupported"
    );
    let offset = u32_at(16) as u64;
    let length = u32_at(12) as u64;
    ensure!(
        offset.checked_add(length) == Some(end),
        "invalid ZIP directory bounds or unsupported ZIP64"
    );
    file.seek(SeekFrom::Start(offset))?;
    let mut names = HashSet::new();
    let mut consumed = 0u64;
    for _ in 0..count {
        let mut header = [0; 46];
        file.read_exact(&mut header)?;
        ensure!(
            header[..4] == *b"PK\x01\x02",
            "invalid ZIP central-directory record"
        );
        ensure!(header[8] & 1 == 0, "encrypted ZIP entries are unsupported");
        let name_len = u16::from_le_bytes([header[28], header[29]]) as usize;
        let extra_len = u16::from_le_bytes([header[30], header[31]]) as u64;
        let comment_len = u16::from_le_bytes([header[32], header[33]]) as u64;
        ensure!(
            name_len > 0 && name_len <= 1024,
            "ZIP entry name exceeds 1024 bytes or is empty"
        );
        consumed = consumed
            .checked_add(46 + name_len as u64 + extra_len + comment_len)
            .context("ZIP directory overflow")?;
        ensure!(consumed <= length, "ZIP directory entry exceeds bounds");
        let mut name = vec![0; name_len];
        file.read_exact(&mut name)?;
        ensure!(names.insert(name), "duplicate ZIP central-directory name");
        for p in [20, 24, 42] {
            ensure!(
                header[p..p + 4] != [255; 4],
                "ZIP64 entries are unsupported"
            );
        }
        file.seek(SeekFrom::Current((extra_len + comment_len) as i64))?;
    }
    ensure!(consumed == length, "unexpected ZIP directory data");
    file.seek(SeekFrom::Start(0))?;
    Ok(count)
}
pub struct Archive {
    zip: zip::ZipArchive<File>,
}
impl Archive {
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        ensure!(
            file.metadata()?.is_file(),
            "PXO input must be a regular file"
        );
        ensure!(
            file.metadata()?.len() <= MAX_ARCHIVE,
            "ZIP exceeds 512 MiB compressed"
        );
        let count = check_directory(&mut file)?;
        let mut zip = zip::ZipArchive::new(file).context("invalid ZIP archive")?;
        ensure!(zip.len() == count, "duplicate decoded ZIP names");
        let mut names = HashSet::new();
        let mut total = 0u64;
        for i in 0..zip.len() {
            let entry = zip.by_index(i)?;
            let name = entry.name();
            ensure!(
                !name.is_empty() && !name.starts_with('/') && !name.contains(['\\', ':', '\0']),
                "unsafe ZIP path {name:?}"
            );
            let path = name.strip_suffix('/').unwrap_or(name);
            ensure!(
                path.split('/')
                    .all(|c| !c.is_empty() && c != "." && c != ".."),
                "unsafe ZIP path {name:?}"
            );
            ensure!(
                names.insert(name.to_owned()),
                "duplicate ZIP entry {name:?}"
            );
            if let Some(mode) = entry.unix_mode() {
                let kind = mode & 0o170000;
                ensure!(
                    kind == 0 || kind == 0o100000 || (kind == 0o040000 && entry.is_dir()),
                    "unsupported ZIP special file {name}"
                );
            }
            ensure!(entry.size() <= MAX_ENTRY, "ZIP entry {name} exceeds 64 MiB");
            total = total
                .checked_add(entry.size())
                .context("ZIP size overflow")?;
            ensure!(total <= MAX_EXPANDED, "ZIP exceeds 8 GiB decompressed");
        }
        Ok(Self { zip })
    }
    pub fn contains(&self, name: &str) -> bool {
        self.zip.index_for_name(name).is_some()
    }
    pub fn read(&mut self, name: &str, limit: u64) -> Result<Vec<u8>> {
        ensure!(limit <= MAX_ENTRY, "entry read limit exceeds policy");
        let entry = self
            .zip
            .by_name(name)
            .with_context(|| format!("missing ZIP entry {name}"))?;
        let size = entry.size();
        ensure!(
            size <= limit,
            "ZIP entry {name} exceeds allowed size {limit}"
        );
        let mut bytes = Vec::with_capacity(size as usize);
        entry
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .with_context(|| format!("read ZIP entry {name}"))?;
        ensure!(
            bytes.len() as u64 <= limit && bytes.len() as u64 == size,
            "invalid decompressed length for {name}"
        );
        Ok(bytes)
    }
}
