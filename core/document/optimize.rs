//! Bounded exact-index JSON optimization. See docs/optimizer.md for search limits.
use crate::{
    encoding::{self, Kind, Placement},
    output,
    schema::{self, CelDefinition},
};
use anyhow::{Result, ensure};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Report {
    pub before: usize,
    pub minified_before: usize,
    pub after: usize,
    pub evaluated: usize,
    pub skipped: usize,
}

fn image(cel: &CelDefinition, doc: &schema::Document) -> Result<Vec<Vec<u32>>> {
    match cel {
        CelDefinition::Legacy(c) => Ok(c.pixels.clone()),
        CelDefinition::Encoded(c) => c.decode(&doc.canvas, doc.palette.len()),
    }
}

/// Compare document fields and exact indices without passing through RGBA.
pub fn verify(before: &[u8], after: &[u8]) -> Result<()> {
    let a = schema::read_document(before)?;
    let b = schema::read_document(after)?;
    ensure!(a.cels.len() == b.cels.len(), "cel count changed");
    for (x, y) in a.cels.iter().zip(&b.cels) {
        ensure!(
            x.id() == y.id() && image(x, &a)? == image(y, &b)?,
            "cel indices or ID changed"
        );
    }
    let mut av: Value = serde_json::from_slice(before)?;
    let mut bv: Value = serde_json::from_slice(after)?;
    av.as_object_mut().unwrap().remove("cels");
    bv.as_object_mut().unwrap().remove("cels");
    ensure!(av == bv, "document semantics changed");
    Ok(())
}

pub fn optimize(bytes: &[u8]) -> Result<(Vec<u8>, Report)> {
    let doc = schema::read_document(bytes)?;
    let mut value: Value = serde_json::from_slice(bytes)?;
    let minified_before = serde_json::to_vec(&value)?.len();
    let mut report = Report {
        before: bytes.len(),
        minified_before,
        after: 0,
        evaluated: 0,
        skipped: 0,
    };
    // A deterministic per-cel allowance also caps aggregate work across the document.
    let allowance = 128 * 1024 * 1024usize / doc.cels.len().max(1);
    for (n, cel) in doc.cels.iter().enumerate() {
        let original = &value["cels"][n];
        let mut best = serde_json::to_vec(original)?;
        report.evaluated += 1;
        // Version 1 has only legacy grids. Keep its version and representation contract.
        if doc.version == 1 {
            continue;
        }
        let pixels = image(cel, &doc)?;
        let (w, h) = (doc.canvas.width, doc.canvas.height);
        let mut counts = BTreeMap::<u32, usize>::new();
        for &i in pixels.iter().flatten() {
            *counts.entry(i).or_default() += 1;
        }
        let mut defaults: Vec<_> = counts.into_iter().collect();
        defaults.sort_by_key(|&(i, count)| (std::cmp::Reverse(count), i));
        defaults.truncate(3);
        let mut choices: Vec<u32> = defaults.into_iter().map(|(i, _)| i).collect();
        if !choices.contains(&0) {
            choices.push(0);
        }
        let mut regions = vec![Placement::default()];
        for &default in &choices {
            let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
            for (y, row) in pixels.iter().enumerate() {
                for (x, &i) in row.iter().enumerate() {
                    if i != default {
                        x0 = x0.min(x as u32);
                        y0 = y0.min(y as u32);
                        x1 = x1.max(x as u32 + 1);
                        y1 = y1.max(y as u32 + 1);
                    }
                }
            }
            if x1 == 0 {
                x0 = 0;
                y0 = 0;
                x1 = 1;
                y1 = 1;
            }
            if x1 - x0 != w || y1 - y0 != h {
                regions.push(Placement {
                    offset: Some([x0, y0]),
                    size: Some([x1 - x0, y1 - y0]),
                    default_index: Some(default),
                });
            }
        }
        let kinds = [
            Kind::Grid,
            Kind::HybridRle,
            Kind::SparseRows,
            Kind::RowTable,
            Kind::Tiles { tile_size: [2, 2] },
            Kind::Tiles { tile_size: [4, 4] },
            Kind::Tiles { tile_size: [8, 8] },
            Kind::Tiles {
                tile_size: [16, 16],
            },
        ];
        let mut work = 0usize;
        for region in regions {
            for kind in kinds {
                let [rw, rh] = region.size.unwrap_or([w, h]);
                let needs_default = region.size.is_some()
                    || matches!(kind, Kind::SparseRows)
                    || matches!(kind, Kind::Tiles { tile_size:[tw,th] } if rw%tw != 0 || rh%th != 0);
                let ds: Vec<Option<u32>> = if region.size.is_some() {
                    vec![region.default_index]
                } else if needs_default {
                    choices.iter().copied().map(Some).collect()
                } else {
                    vec![None]
                };
                for default_index in ds {
                    // Includes full-image writer validation, region construction and tile padding.
                    let cost = w as usize * h as usize + (rw as usize + 16) * (rh as usize + 16);
                    if work.saturating_add(cost) > allowance {
                        report.skipped += 1;
                        continue;
                    }
                    work += cost;
                    let candidate = encoding::encode(
                        cel.id().into(),
                        &pixels,
                        &doc.canvas,
                        doc.palette.len(),
                        kind,
                        Placement {
                            default_index,
                            ..region
                        },
                    );
                    let Ok(candidate) = candidate else {
                        report.skipped += 1;
                        continue;
                    };
                    let candidate = serde_json::to_value(candidate)?;
                    let encoded = serde_json::to_vec(&candidate)?;
                    report.evaluated += 1;
                    // Length then lexical bytes: independent of original encoding and search order.
                    if (encoded.len(), &encoded) < (best.len(), &best) {
                        best = encoded;
                    }
                }
            }
        }
        value["cels"][n] = serde_json::from_slice(&best)?;
    }
    let mut result = serde_json::to_vec(&value)?;
    // Individually valid dictionaries can exceed cumulative document budgets together.
    // Fall back to the original document, still minified, in that case.
    if schema::read_document(&result).is_err() {
        result = serde_json::to_vec(&serde_json::from_slice::<Value>(bytes)?)?;
    }
    verify(bytes, &result)?;
    report.after = result.len();
    Ok((result, report))
}

pub fn destination(path: &Path) -> PathBuf {
    if path.to_string_lossy().ends_with(".pixen.json") {
        path.to_owned()
    } else {
        let mut name = path.as_os_str().to_os_string();
        name.push(".pixen.json");
        name.into()
    }
}

pub fn run(
    input: &Path,
    output_path: Option<&Path>,
    in_place: bool,
    force: bool,
) -> Result<Report> {
    ensure!(
        !crate::input::is_container(input),
        "optimize requires self-contained JSON; unpack the container, optimize JSON, then pack (or pack JSON --optimize)"
    );
    ensure!(
        in_place != output_path.is_some(),
        "require --output or --in-place"
    );
    let destination = output_path
        .map(destination)
        .unwrap_or_else(|| input.to_owned());
    if in_place {
        let meta = fs::symlink_metadata(input)?;
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink(),
            "in-place input must be a regular non-symlink file"
        );
        output::resolved(input)?;
    } else {
        output::preflight(input, &[&destination], force)?;
    }
    // Shared lock protects the snapshot; final bytes/metadata check detects observed edits.
    let source = fs::OpenOptions::new().read(true).open(input)?;
    if in_place {
        source.try_lock_shared()?;
    }
    let snapshot_meta = source.metadata()?;
    let before = output::read_bounded(input, schema::MAX_JSON)?;
    let (bytes, report) = optimize(&before)?;
    let prepared = output::Prepared::new(&destination, |file| {
        file.write_all(&bytes)?;
        file.seek(SeekFrom::Start(0))?;
        let mut written = Vec::new();
        file.take(schema::MAX_JSON + 1).read_to_end(&mut written)?;
        verify(&before, &written)
    })?;
    if in_place {
        check_source(input, &snapshot_meta, &before)?;
    }
    drop(source); // Windows replacement requires releasing the source lock.
    prepared.commit(in_place || force)?;
    Ok(report)
}

fn check_source(input: &Path, snapshot: &fs::Metadata, before: &[u8]) -> Result<()> {
    let current = fs::symlink_metadata(input)?;
    ensure!(
        current.is_file()
            && !current.file_type().is_symlink()
            && current.modified()? == snapshot.modified()?
            && output::read_bounded(input, schema::MAX_JSON)? == before,
        "source changed during optimization"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concurrent_edit_and_failed_temp_verification_leave_source_intact() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("art.pixen.json");
        let original = include_bytes!("../../skills/pixen/examples/encodings.pixloom.json");
        fs::write(&path, original).unwrap();
        let snapshot = fs::metadata(&path).unwrap();
        let temp = output::Prepared::new(&path, |file| {
            file.write_all(b"corrupted temporary output")?;
            verify(original, b"corrupted temporary output")
        });
        assert!(temp.is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        let prepared = output::Prepared::new(&path, |file| {
            file.write_all(original)?;
            Ok(())
        })
        .unwrap();
        fs::write(&path, b"concurrent edit").unwrap();
        assert!(check_source(&path, &snapshot, original).is_err());
        drop(prepared);
        assert_eq!(fs::read(&path).unwrap(), b"concurrent edit");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
