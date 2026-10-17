# Container benchmarks

This is the pre-optimization baseline. See [reader optimizations and current measurements](container-reader-optimizations.md) for the implemented changes.

Measured locally on Windows x64, 2026-10-17, release build with the locked dependency set.

Reproduce with `cargo run --release --locked --example container_bench`.
Raw measurements: [container-benchmark-results.jsonl](container-benchmark-results.jsonl).

Each fixture is generated deterministically with exact palette indices (including
duplicate RGBA palette entries). Dense/sparse/repeated-row/tiled fixtures have
32 cels on a 128x128 canvas; animated has 32 frames; many-small has 2,048
4x4 cels. Static fixtures retain 31 unused cels deliberately. Row and tile
dictionaries remain local. Compression baseline is one deflate-level-6 ZIP entry
containing the exact same compact, self-contained document; no alternative
architecture is selected from this comparison.

Each loading metric averages five warm runs in a separate worker process.
Full load includes semantic validation and RGBA compilation of every cel.
First-frame loading opens structure and decodes exact indices for cels referenced
by frame 1 only, without compositing or full validation. Peak memory is Windows
PeakWorkingSet64 for that worker, including runtime/allocator and file buffers,
not heap-only or an isolated operation delta. The PowerShell query runs after
timing. Non-Windows runs report null for peak memory. These are small synthetic
fixtures, not cold-cache or statistical performance guarantees.

| Fixture | JSON bytes | Split ZIP | Single-document ZIP | JSON full ms | ZIP full ms | First-frame indices ms | ZIP full peak MiB | Frame peak MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| dense | 1,058,952 | 205,802 | 198,656 | 22.39 | 29.10 | 3.60 | 10.08 | 6.38 |
| sparse | 172,008 | 25,508 | 16,120 | 7.65 | 11.36 | 1.67 | 8.04 | 5.63 |
| rows | 19,368 | 6,752 | 709 | 3.06 | 4.72 | 1.51 | 7.50 | 5.47 |
| tiled | 25,096 | 7,616 | 775 | 5.71 | 7.08 | 1.65 | 7.46 | 5.57 |
| animated | 1,047,946 | 212,325 | 203,957 | 27.29 | 32.42 | 2.59 | 15.24 | 6.56 |
| many-small | 178,852 | 373,742 | 24,159 | 5.25 | 42.25 | 22.61 | 11.31 | 11.30 |

Split compression trades cross-cel compression for independent entry access.
The many-small fixture costs 349,583 additional bytes versus the one-entry
baseline (about 171 bytes per cel, including headers, references and compression
differences). Repeated rows/tiles also compress better across cels in the baseline.
Full container loading is slower here; selective loading avoids unrelated pixel
payloads but still indexes the entire ZIP and validates document structure.

Current CLI build/export/render fully load and validate authoring containers,
including unused cels; lazy access is exposed through Reader::open/load_cel/
decode_cel and inspect. No frame-delta compression or shared cross-cel dictionary
is introduced. Deterministic bytes are tested with the current locked ZIP/deflate
implementation; identical output across future compressor versions is not promised.

A follow-up [stage profile](container-profile.md) identifies where full loading
time is spent and prioritizes potential optimizations without changing production code.
