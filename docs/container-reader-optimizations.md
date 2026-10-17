# Container reader optimizations

Implemented and measured locally on Windows x64, 2026-10-17. Container layout,
schema versions, encodings, writer output and validation policy remain unchanged.

## Changes

- Cel payloads deserialize directly into the existing Grid, HybridRle, SparseRows,
  RowTable, Tiles and LegacyCel structs. The existing streaming preflight also
  identifies the encoding and rejects payload IDs. A serde map adapter supplies
  the structural ID and consumes the already checked encoding header. There is
  no intermediate pixel-bearing JSON Value tree or CelDefinition variant trial.
- Archive reads use a 64 KiB buffer, including the duplicate-name directory check.
  Skipping ZIP extra/comment bytes preserves buffered data. The existing entry
  safety pass uses raw entry metadata, checks compression explicitly, and caches
  the inventory. Asking for inventory no longer reopens every ZIP entry.
- Full document loading walks references once. Individual cel loading uses an
  ID-to-reference index. References are exposed through the read-only
  `Reader::references()` accessor so callers cannot invalidate that index.

Duplicate fields/names, unsupported compression/encryption, unknown fields,
resource budgets, CRC/decompression checks, missing/multiply assigned payloads,
explicit references and unused cel validation remain enforced. The reader still
validates before compiling; this change does not remove validation passes.

## Uninstrumented before/after

Both snapshots use the existing six synthetic fixtures, release builds and five
warm runs per worker. Final measurements ran after the test suite completed.
They are local observations, not statistical guarantees. The old snapshot is
[retained](container-benchmark-results.jsonl); the new snapshot is
[container-benchmark-optimized-results.jsonl](container-benchmark-optimized-results.jsonl).
Every fixture has the same JSON size, split ZIP size and baseline ZIP size.

| Fixture | Full before ms | Full after ms | Time reduction | First-frame before ms | First-frame after ms | Full peak before/after MiB |
|---|---:|---:|---:|---:|---:|---:|
| dense | 29.10 | 12.18 | 58.1% | 3.60 | 0.93 | 10.08 / 9.82 |
| sparse | 11.36 | 5.07 | 55.4% | 1.67 | 0.67 | 8.04 / 8.57 |
| rows | 4.72 | 3.76 | 20.4% | 1.51 | 0.51 | 7.50 / 7.52 |
| tiled | 7.08 | 6.48 | 8.5% | 1.65 | 0.63 | 7.46 / 7.51 |
| animated | 32.42 | 19.79 | 38.9% | 2.59 | 1.20 | 15.24 / 13.92 |
| many-small | 42.25 | 31.71 | 24.9% | 22.61 | 17.08 | 11.31 / 11.30 |

First-frame metrics measure selective index loading, not compositing. Peak memory
is process peak working set, including allocator/runtime effects; small differences
are not evidence of a reliable memory reduction. CLI build/export/render still
fully validate and load all declared cels, including unused definitions.

## Follow-up stage profile

Run `python scripts/profile_container.py` for the current reader. It instruments
an isolated source copy and writes
[container-profile-optimized-results.jsonl](container-profile-optimized-results.jsonl).
The [original profile](container-profile.md) is retained as a historical baseline.
Stage timers introduce overhead; use the uninstrumented table above for overall
speed comparisons. Compile validation is nested inside compile.total_inclusive.

| Fixture | Payload typed parse ms | Preflight ms | Payload read/inflate/CRC ms | Archive setup ms | Compile ms |
|---|---:|---:|---:|---:|---:|
| dense | 5.78 | 4.48 | 2.07 | 0.30 | 1.76 |
| sparse | 0.82 | 1.64 | 0.50 | 0.22 | 1.14 |
| rows | 0.18 | 0.13 | 1.16 | 0.25 | 3.43 |
| tiled | 0.20 | 0.10 | 0.61 | 0.19 | 5.37 |
| animated | 10.55 | 3.82 | 1.64 | 0.24 | 2.73 |
| many-small | 1.10 | 1.09 | 11.98 | 12.12 | 1.09 |

The generic JSON conversion stage is gone, and full loading performs no cel-ID
lookups. Many-small still pays for ZIP indexing and 2,048 entry reads: the profile
records about 11 ms indexing and 12 ms reading/inflating/checking payloads. Row
and tile fixtures still spend most time compiling their encoded pixels. Further
work in those areas requires separate evidence and regression coverage.

## Verification

- `cargo test --locked`: 103 passed, 0 failed, 5 existing tests ignored.
- `cargo clippy --all-targets --locked -- -D warnings`: passed.
- `cargo fmt --check`, release build and `git diff --check`: passed.
- Added header-order/strict-field coverage for direct typed loading; strengthened
  unsupported-compression coverage for the shared native archive reader.
- Existing semantic/index round trips, lazy reads, deterministic output,
  malformed/resource-limit tests and JSON/container native/export equivalence pass.

Implementation files: src/container.rs, src/container/payload.rs, src/archive.rs
and src/encoding/preflight.rs. Updated tests/container.rs and the profiling script;
the benchmark example uses the read-only reference accessor where needed.
No dependency changes, commits, publication, global installation or changes to
consuming projects.
