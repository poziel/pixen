//! Split-cel ZIP authoring storage; see docs/container-format.md.
mod payload;

use crate::{
    archive::Archive,
    output,
    schema::{self, CelDefinition, Document},
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    io::{Seek, Write},
    path::Path,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: String,
    container_version: u32,
    document: String,
    required_features: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub id: String,
    pub payload: String,
}
pub struct Reader {
    archive: Archive,
    /// Existing Document fields with cel definitions replaced by explicit references.
    pub structure: Value,
    references: Vec<Reference>,
    reference_index: HashMap<String, usize>,
    document: Document,
}
fn json_read(archive: &mut Archive, name: &str) -> Result<Value> {
    let bytes = archive.read(name, crate::archive::MAX_ENTRY)?;
    crate::encoding::preflight::check(&bytes)?;
    serde_json::from_slice(&bytes).with_context(|| format!("invalid JSON entry {name}"))
}
impl Reader {
    pub fn open(path: &Path) -> Result<Self> {
        let mut archive = Archive::open(path)?;
        let inventory = archive.inventory()?;
        ensure!(
            inventory.len() <= schema::MAX_SLOTS + 2,
            "container entry count exceeds limit"
        );
        ensure!(
            inventory.iter().map(|(_, n)| n).sum::<u64>() <= schema::MAX_JSON,
            "container expanded JSON exceeds limit"
        );
        let manifest: Manifest = serde_json::from_value(json_read(&mut archive, "manifest.json")?)?;
        ensure!(manifest.format == "spritex", "unsupported container format");
        ensure!(
            manifest.container_version == 1,
            "unsupported container version {}",
            manifest.container_version
        );
        ensure!(
            manifest.required_features.is_empty(),
            "unsupported required container features: {:?}",
            manifest.required_features
        );
        ensure!(
            manifest.document == "document.json",
            "invalid document reference"
        );
        let structure = json_read(&mut archive, &manifest.document)?;
        let references: Vec<Reference> =
            serde_json::from_value(structure.get("cels").context("missing cels")?.clone())?;
        ensure!(references.len() <= schema::MAX_SLOTS, "too many cels");
        let mut expected: HashSet<String> = ["manifest.json".into(), "document.json".into()].into();
        let mut ids = HashSet::new();
        for r in &references {
            schema::identifier(&r.id)?;
            ensure!(ids.insert(&r.id), "duplicate cel ID");
            let name = r
                .payload
                .strip_prefix("cels/")
                .context("invalid payload reference")?;
            ensure!(
                !name.contains('/') && name.ends_with(".json") && name.len() > 5,
                "invalid payload reference"
            );
            ensure!(
                expected.insert(r.payload.clone()),
                "multiply assigned cel payload"
            );
            ensure!(
                archive.contains(&r.payload),
                "missing cel payload {}",
                r.payload
            );
        }
        ensure!(
            inventory.len() == expected.len()
                && inventory.iter().all(|(n, _)| expected.contains(n)),
            "unexpected container entry"
        );
        // Reuse the semantic model for structural checks, with zero-storage grid placeholders.
        let mut check = structure.clone();
        let version = check["version"]
            .as_u64()
            .context("missing authoring version")?;
        ensure!(
            matches!(version, 1 | 2),
            "unsupported authoring version {version}"
        );
        check["cels"] = Value::Array(
            references
                .iter()
                .map(|r| json!({"id":r.id,"pixels":[]}))
                .collect(),
        );
        let doc: Document = serde_json::from_value(check)?;
        schema::validate_structure(&doc)?;
        let reference_index = references
            .iter()
            .enumerate()
            .map(|(i, r)| (r.id.clone(), i))
            .collect();
        Ok(Self {
            reference_index,
            archive,
            structure,
            references,
            document: doc,
        })
    }
    pub fn references(&self) -> &[Reference] {
        &self.references
    }
    pub fn load_cel(&mut self, id: &str) -> Result<CelDefinition> {
        let index = *self.reference_index.get(id).context("unknown cel ID")?;
        let r = &self.references[index];
        let bytes = self.archive.read(&r.payload, crate::archive::MAX_ENTRY)?;
        payload::read(&bytes, &r.id, self.document.version)
            .with_context(|| format!("invalid cel payload {}", r.payload))
    }
    /// Expand only the requested cel to exact palette indices.
    pub fn decode_cel(&mut self, id: &str) -> Result<Vec<Vec<u32>>> {
        let cel = self.load_cel(id)?;
        match cel {
            CelDefinition::Encoded(c) => {
                ensure!(
                    self.document.version == 2,
                    "version 1 requires legacy grid cels"
                );
                c.decode(&self.document.canvas, self.document.palette.len())
            }
            CelDefinition::Legacy(c) => {
                ensure!(
                    self.document.version == 1,
                    "version 2 requires explicit encoding"
                );
                ensure!(
                    c.pixels.len() == self.document.canvas.height as usize,
                    "wrong grid height"
                );
                for row in &c.pixels {
                    ensure!(
                        row.len() == self.document.canvas.width as usize,
                        "wrong grid width"
                    );
                    ensure!(
                        row.iter()
                            .all(|&i| (i as usize) < self.document.palette.len()),
                        "invalid palette index"
                    );
                }
                Ok(c.pixels)
            }
        }
    }
    pub fn load(mut self) -> Result<Document> {
        let mut cels = Vec::with_capacity(self.references.len());
        for r in &self.references {
            let bytes = self.archive.read(&r.payload, crate::archive::MAX_ENTRY)?;
            cels.push(
                payload::read(&bytes, &r.id, self.document.version)
                    .with_context(|| format!("invalid cel payload {}", r.payload))?,
            );
        }
        let mut doc = self.document;
        doc.cels = cels;
        schema::validate_document(&doc)?;
        Ok(doc)
    }
}
pub fn write<W: Write + Seek>(writer: W, doc: &Document) -> Result<()> {
    schema::validate_document(doc)?;
    let mut structure = serde_json::to_value(doc)?;
    structure["cels"] = Value::Array(
        doc.cels
            .iter()
            .map(|c| json!({"id":c.id(),"payload":format!("cels/{}.json",c.id())}))
            .collect(),
    );
    let mut zip = zip::ZipWriter::new(writer);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(6))
        .last_modified_time(zip::DateTime::default());
    let mut entry = |name: &str, value: &Value| -> Result<()> {
        let bytes = serde_json::to_vec(value)?;
        ensure!(
            bytes.len() as u64 <= crate::archive::MAX_ENTRY,
            "container entry exceeds limit"
        );
        zip.start_file(name, options)?;
        zip.write_all(&bytes)?;
        Ok(())
    };
    entry(
        "manifest.json",
        &json!({"format":"spritex","container_version":1,"document":"document.json","required_features":[]}),
    )?;
    entry("document.json", &structure)?;
    let mut cels: Vec<_> = doc.cels.iter().collect();
    cels.sort_by_key(|c| c.id());
    for cel in cels {
        let mut payload = serde_json::to_value(cel)?;
        payload.as_object_mut().unwrap().remove("id");
        entry(&format!("cels/{}.json", cel.id()), &payload)?;
    }
    zip.finish()?;
    Ok(())
}
pub fn pack(input: &Path, destination: &Path, optimize: bool, force: bool) -> Result<()> {
    output::preflight(input, &[destination], force)?;
    ensure!(
        destination.extension().is_some_and(|e| e == "spritex"),
        "pack output must end in .spritex"
    );
    let mut doc = crate::input::read(input)?;
    if optimize {
        doc = schema::read_document(&crate::optimize::optimize(&serde_json::to_vec(&doc)?)?.0)?;
    }
    let prepared = output::Prepared::new(destination, |file| {
        write(
            output::LimitedFile::new(file, output::MAX_OUTPUT_BYTES),
            &doc,
        )
    })?;
    let loaded = Reader::open(prepared.staged_path())?.load()?;
    ensure!(
        serde_json::to_value(&doc)? == serde_json::to_value(loaded)?,
        "staged container changed document"
    );
    prepared.commit(force)
}
