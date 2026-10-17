# Container phase verification

Verified locally on Windows, 2026-10-17.

- `cargo test --locked`: 103 passed, 0 failed, 5 ignored.
- `cargo clippy --all-targets --locked -- -D warnings`: passed.
- `cargo fmt --check`: passed.
- `cargo build --release --locked`: passed.
- `git diff --check`: passed (Git reports configured LF/CRLF conversion notices).
- `cargo run --release --locked --example container_bench`: completed all six
  fixtures; raw JSONL and interpreted results are repository-local.

The five ignored tests are the existing mixed video CLI test, two installed-FFmpeg
video tests, and two original native-reference tests requiring external fixtures.
No new container test is ignored. The test suite's expected negative CLI test
prints a missing `--output` error; that test passes.

Thirteen container tests cover all five encodings in a mixed document, legacy
round trips, unused definitions, metadata, explicit edit links, deterministic
bytes, on-demand indices, structural inspection with corrupt unrelated storage,
explicit arbitrary payload filenames/archive ordering, exact optimized indices,
JSON/container PNG/SVG/GIF equivalence and decoded PXO/Aseprite equivalence,
reference/version/feature errors, unexpected and unsafe entries, encryption,
unsupported compression, duplicate ZIP names/JSON fields, compressed/entry/total
expanded/count limits, malformed/truncated archives, overwrite protections,
failed forced unpack preserving destination and source, and actionable standalone
optimize rejection.

Compatibility decisions:

- Container version 1 is independent of authoring versions 1 and 2. Released
  versions follow backward-reader compatibility. Unsupported versions/features
  fail explicitly. Existing native-project workflows and legacy JSON filenames
  remain supported.
- Container fields reuse existing Document/CelDefinition types. External JSON
  embeds all payloads. No new editing-link semantics or cross-cel dictionaries.
- Only referenced JSON entries plus manifest/document are accepted. No extraction.
- Pack accepts either authoring input and can safely repack; standalone optimize
  requires JSON and explains the unpack/optimize/pack workflow.

Limitations:

- CLI build/export/render fully validate and load all cels; selective payload
  access is available through Reader and metadata inspection through inspect.
- Single-frame benchmark measures index loading, not compositing/export; memory
  is process peak working set and timings use small synthetic warm-cache fixtures.
- Split-entry ZIP overhead is material for tiny cels; JSON optimization does not
  guarantee smallest ZIP. Determinism is within the locked compressor version.
- Existing native-format representability limits still apply, including Aseprite
  rejection of unused definitions. Container retention does not remove those limits.

Changed implementation: src/container.rs, src/container/payload.rs, src/input.rs, src/archive.rs,
src/schema.rs, src/encoding/preflight.rs, src/output.rs, src/operations.rs,
src/optimize.rs, src/main.rs and src/lib.rs. Added tests/container.rs and
examples/container_bench.rs. Updated README and both local companion skill/reference
files. Added container schema, implementation plan, benchmark report/raw measurements
and this verification record under docs/. No dependency changes, commits,
publication, global installation, asset renames or consuming-project modifications.

Reader optimization follow-up and current measurements: [container-reader-optimizations.md](container-reader-optimizations.md).
