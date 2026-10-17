# Container loading hotspot profile

This is the pre-optimization baseline. See [reader optimizations and current measurements](container-reader-optimizations.md) for the implemented changes.

Measured on Windows x64, 2026-10-17, using release builds and the same six
deterministic fixtures as [the container benchmark](container-benchmarks.md).

Reproduce with `python scripts/profile_container.py`. The script instruments an
isolated temporary copy under target/, builds it into target/container-profile-build/,
and removes the source copy afterward. Production source, format and behavior
remain unchanged. No profiling dependency or global installation is needed.

Raw measurements: [container-profile-results.jsonl](container-profile-results.jsonl).

This is stage timing, not a sampled CPU flame graph. Each metric averages five
warm loads in a separate worker. Timers and thread-local counter bookkeeping add
overhead, especially for many tiny entries; use the original uninstrumented
benchmark for end-to-end latency. Read/inflate/CRC combines file reads, ZIP entry
setup, decompression, checksum verification and buffer allocation; it does not
isolate decompression CPU time. JSON-to-typed conversion includes ID insertion
and typed deserialization. Compile includes its own validation, decoded pixel
expansion and RGBA construction; its nested validation metric must not be summed
again. Unlisted time includes structural construction, allocations, destruction,
reference checks and instrumentation overhead.

| Fixture | Total ms | Archive setup ms | Payload reads ms | JSON preflight ms | JSON parsing ms | JSON-to-typed ms | Cel lookup ms | Compile ms |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| dense | 29.23 | 0.53 | 2.36 | 4.00 | 7.83 | 12.65 | 0.01 | 1.01 |
| sparse | 11.21 | 0.57 | 0.70 | 1.77 | 2.92 | 3.57 | 0.01 | 0.96 |
| rows | 4.52 | 0.82 | 0.56 | 0.06 | 0.13 | 0.20 | 0.00 | 2.36 |
| tiled | 7.07 | 0.69 | 0.48 | 0.09 | 0.18 | 0.33 | 0.00 | 4.89 |
| animated | 36.34 | 0.52 | 2.30 | 3.85 | 7.80 | 17.80 | 0.01 | 2.39 |
| many-small | 50.68 | 18.65 | 12.14 | 1.23 | 1.47 | 3.79 | 4.31 | 1.24 |

## Findings and next optimization targets

1. Dense and animated cels spend about 70% of total time parsing into generic
   JSON Values and then converting those Values into CelDefinition. The conversion
   alone takes 12.65 ms (dense) and 17.80 ms (animated). Reading/inflate/CRC takes
   only 2.36 and 2.30 ms respectively. First priority: investigate direct typed
   payload deserialization using the known authoring version, avoiding the generic
   Value intermediate and unnecessary untagged variant trials. Keep duplicate-field
   rejection, unknown-field rejection and the streaming resource preflight.
2. Many-small spends 18.65 ms in archive setup and 12.14 ms reading 2,048 payloads.
   Indexing is 8.76 ms; classic-directory safety checking is 4.72 ms; entry policy
   checking and inventory total 5.17 ms. Investigate buffered directory reads and
   collecting reusable entry metadata during the existing safety pass. Preserve
   raw duplicate-name checks, compressed/expanded limits and strict entry policy.
3. Many-small cel lookup takes 4.31 ms. Full loading repeatedly searches the
   reference list, creating quadratic work. Iterate explicit references directly
   for full loading; use an ID index for on-demand access. This is measurable but
   smaller than archive setup and per-entry reads.
4. Row-table and tiled fixtures spend 2.36/4.89 ms compiling pixels. That is
   52%/69% of total loading time and belongs to the shared encoding/RGBA pipeline,
   not specifically ZIP decompression. Optimize it only if full-load rendering
   latency warrants it; lazy metadata access already avoids that work.
5. Full document validation before compilation and validation inside compilation
   are repeated. Here each is under 0.53 ms; removing that repetition should not
   be the first optimization and would require a trustworthy validated-document
   boundary.

These are measured priorities, not promised speedups. No production optimization
has been applied in this profiling step. In particular, the results do not justify
weakening validation or changing the split-cel container architecture. JSON remains
useful as reviewable version-controlled authoring data; the container remains useful
for portable handoff, storage and selective access. Many tiny cels are still a case
where the ZIP may be larger than standalone JSON.
