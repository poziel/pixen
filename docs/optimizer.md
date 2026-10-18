# Lossless JSON optimization

```
pixloom optimize art.pixloom.json --output art-small
pixloom optimize art.pixloom.json --in-place
```

A destination is mandatory. Separate outputs append `.pixloom.json` unless already
present; their parent must exist. Existing files require `--force`, and the usual
output alias and symlink protections apply. `--force` is incompatible with
`--in-place`. In-place retains the source filename.

Author whichever valid encoding is convenient, then optimize. Grids are often the
easiest starting point; cropped encodings and local dictionaries remain useful for
authoring. Do not manually predict which representation is smallest.

## Search contract

For each version-2 cel, decode root-palette indices directly and retain its original
representation as a candidate. Evaluate full-canvas grid, hybrid-rle, sparse-rows,
row-table, and square tiles of 2, 4, 8 and 16 pixels. Default choices are the three
most frequent indices (ties by ascending index), plus index zero if absent. For
each default also evaluate the tight bounding rectangle of non-default pixels;
a uniform image uses a 1x1 rectangle. Full-sized rectangles are omitted. Sparse
omissions and tile padding use those same default choices. Crops retain their
outside default. Encodings that require no default omit it.

Writers use strict byte-saving horizontal runs, contiguous non-default sparse
spans, and first-occurrence local row/tile dictionaries. There is no search over
alternative sparse span joins, dictionary permutations, arbitrary rectangles,
rectangular tile sizes, or every palette default. No frame deltas, palette
reordering, cel merging or editing-link changes are performed. This is a bounded
candidate search, **not a proof of global mathematical optimality**.

Every successfully generated candidate is validated and measured as actual
minified JSON, including ID, encoding, placement, default, dictionaries, maps and
references. Ties use lexicographic serialized bytes after sorted JSON object
keys, making results deterministic and byte-idempotent. Each cel is no larger
than its original minified representation. Version 1 retains legacy grids and
its version; only JSON minification applies to that format.

The search order is full canvas followed by crops in default order; within each
region: grid, hybrid-rle, sparse-rows, row-table, then ascending tile sizes.
A per-cel work allowance is floor(128 Mi / cel count) pixel-work units. Before
construction each candidate charges canvas area plus (region width + 16) times
(region height + 16), covering full-image validation and padded tile traversal.
Candidates exceeding the allowance or encoding budgets are skipped. Original
candidates are always evaluated. Frequency and crop scans add a fixed number of
passes over each image. Existing JSON, canvas, expanded-image, dictionary and
reference limits also apply. Candidates are constructed one at a time; only the
best serialized candidate is retained. The full input/output JSON trees and
validated document are bounded by the existing 256 MiB JSON limit; this is not a
streaming optimizer. If combined selected dictionaries exceed document-wide
budgets, the entire original document is emitted minified instead.

## Verification and reporting

The report separates original file bytes, minified original bytes and final
bytes. Original minus minified original is JSON formatting/normalization savings
(usually whitespace, but JSON escaping/number spelling can also normalize).
Minified original minus final is encoding savings. Candidate counts include the
original for each cel; skips include work-limit and encoding-budget rejections.

All outputs are reparsed and compared for exact cel IDs/order, decoded indices,
and every JSON field outside cel storage. Palette strings/order, metadata,
frames, durations, layers, tags, timelines and explicit links remain unchanged.
A same-directory temporary file is flushed and synced, reread and verified before
installation through the existing atomic output infrastructure. Any preparation
or verification error discards the temporary file. In-place takes a shared source
file lock, then checks source type, modification time and exact bytes immediately
before installation. Windows requires releasing the source handle before atomic
replacement; an uncooperative writer can still race that final release/rename
window. File locks on Unix are advisory. Concurrent edits observed by the final
check abort replacement; this is not a filesystem compare-and-swap primitive.

Rust 1.89 or newer is required for the standard-library file-lock API.
No compressed container is implemented in this phase.
