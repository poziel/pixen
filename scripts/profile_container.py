"""Profile an isolated source copy; never edits production sources. No dependencies.
Run: python scripts/profile_container.py
"""
from pathlib import Path
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="container-profile-", dir=root / "target") as temporary:
    work = Path(temporary)
    shutil.copytree(root / "core", work / "core")
    shutil.copy(root / "Cargo.toml", work)
    shutil.copy(root / "Cargo.lock", work)
    shutil.copy(root / "build.rs", work)
    shutil.copytree(root / "assets", work / "assets")
    # include_bytes! paths in library tests must resolve during all-target checks.
    shutil.copytree(root / "skills", work / "skills")
    (work / "examples").mkdir()
    shutil.copy(root / "examples/container_bench.rs", work / "examples/container_bench.rs")
    (work / "core/profile.rs").write_text('''
use std::{cell::RefCell, collections::BTreeMap, time::Instant};
thread_local! { static COUNTS: RefCell<BTreeMap<&'static str,(u64,u128)>> = const { RefCell::new(BTreeMap::new()) }; }
pub fn record(name: &'static str, start: Instant) {
    let elapsed = start.elapsed().as_nanos();
    COUNTS.with(|counts| { let mut counts=counts.borrow_mut(); let v=counts.entry(name).or_default(); v.0+=1; v.1+=elapsed; });
}
pub fn reset() { COUNTS.with(|c|c.borrow_mut().clear()); }
pub fn snapshot(iterations: u32) -> serde_json::Value {
    COUNTS.with(|c| serde_json::Value::Object(c.borrow().iter().map(|(k,(calls,nanos))|
        ((*k).into(),serde_json::json!({"calls_per_load":*calls as f64/iterations as f64,"ms_per_load":*nanos as f64/1e6/iterations as f64}))).collect()))
}
''')
    with (work / "core/lib.rs").open("a") as file:
        file.write("\npub mod profile;\n")

    def edit(filename, before, after, count=1):
        path = work / filename
        source = path.read_text()
        assert source.count(before) == count, (filename, before, source.count(before))
        path.write_text(source.replace(before, after))

    def time_statement(filename, statement, label):
        edit(filename, statement, 'let profile_start = std::time::Instant::now();\n' + statement + '\ncrate::profile::record("' + label + '", profile_start);')

    time_statement("core/formats/archive.rs", "let count = check_directory(&mut file)?;", "archive.directory_safety")
    time_statement("core/formats/archive.rs", 'let mut zip = zip::ZipArchive::new(file).context("invalid ZIP archive")?;', "archive.zip_index")
    edit("core/formats/archive.rs", "let mut total = 0u64;", "let mut total = 0u64;\nlet profile_start=std::time::Instant::now();")
    edit("core/formats/archive.rs", "        Ok(Self { zip, inventory })", 'crate::profile::record("archive.entry_safety",profile_start);\n        Ok(Self { zip, inventory })')
    time_statement("core/formats/container.rs", "let inventory = archive.inventory()?;", "archive.inventory")
    time_statement("core/formats/container.rs", "let bytes = archive.read(name, crate::archive::MAX_ENTRY)?;", "metadata.read_inflate_crc")
    time_statement("core/formats/container.rs", "crate::encoding::preflight::check(&bytes)?;", "metadata.json_preflight")
    edit("core/formats/container.rs", '    serde_json::from_slice(&bytes).with_context(|| format!("invalid JSON entry {name}"))', '    let profile_start=std::time::Instant::now();\n    let result=serde_json::from_slice(&bytes).with_context(|| format!("invalid JSON entry {name}"));\n    crate::profile::record("metadata.json_value",profile_start);\n    result')
    statement = "let bytes = self.archive.read(&r.payload, crate::archive::MAX_ENTRY)?;"
    edit("core/formats/container.rs", statement, 'let profile_start=std::time::Instant::now();\n'+statement+'\ncrate::profile::record("payload.read_inflate_crc",profile_start);', count=2)
    time_statement("core/formats/container.rs", 'let index = *self.reference_index.get(id).context("unknown cel ID")?;', "payload.reference_lookup")
    time_statement("core/formats/container/payload.rs", "let kind = crate::encoding::preflight::check_cel(bytes)?;", "payload.json_preflight")
    edit("core/formats/container/payload.rs", "    let mut d = serde_json::Deserializer::from_slice(bytes);", "    let profile_start=std::time::Instant::now();\n    let mut d = serde_json::Deserializer::from_slice(bytes);")
    edit("core/formats/container/payload.rs", "    Ok(result)", '    crate::profile::record("payload.typed_json",profile_start);\n    Ok(result)')
    time_statement("core/formats/container.rs", "schema::validate_document(&doc)?;", "document.full_validation")
    edit("core/document/schema.rs", "pub fn compile(doc: Document) -> Result<Sprite> {\n    validate_document(&doc)?;", 'pub fn compile(doc: Document) -> Result<Sprite> {\nlet profile_start=std::time::Instant::now();\n    validate_document(&doc)?;\ncrate::profile::record("compile.validation",profile_start);')
    edit("core/document/schema.rs", "pub fn compile(doc: Document) -> Result<Sprite> {", "pub fn compile(doc: Document) -> Result<Sprite> {\nlet compile_start=std::time::Instant::now();")
    edit("core/document/schema.rs", "    Ok(sprite)\n}", '    crate::profile::record("compile.total_inclusive",compile_start);\n    Ok(sprite)\n}')
    time_statement("core/formats/container.rs", "schema::validate_structure(&doc)?;", "document.structure_validation")
    edit("examples/container_bench.rs", '            "full" => {', '            "full" | "profile" => {')
    edit("examples/container_bench.rs", "    let start = Instant::now();", "    pixen::profile::reset();\n    let start = Instant::now();")
    edit("examples/container_bench.rs", 'json!({"ms":ms,"peak_working_set_bytes":peak()})', 'json!({"ms":ms,"peak_working_set_bytes":peak(),"stages":pixen::profile::snapshot(5)})')
    edit("examples/container_bench.rs", '"container_full":measure("full",&packed)?', '"container_full":measure("profile",&packed)?')
    command = ["cargo", "run", "--release", "--locked", "--manifest-path", str(work / "Cargo.toml"), "--target-dir", str(root / "target/container-profile-build"), "--example", "container_bench"]
    result = subprocess.run(command, cwd=root, check=True, capture_output=True)
    (root / "docs/container-profile-optimized-results.jsonl").write_bytes(result.stdout)
    print(result.stderr.decode(errors="replace"))
    print("Wrote docs/container-profile-optimized-results.jsonl")
