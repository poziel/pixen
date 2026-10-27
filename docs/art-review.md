# Art review APIs

Pixen exposes six read-only review tools through `pixen-mcp`. They provide
reproducible evidence for an AI or a person; they do not rate artistic quality,
call another model, run a critic agent, or repair artwork automatically.

Every review report includes `report_version:1`, `project_id`,
`inspected_revision`, and a source content identity. Images are actual PNG MCP
content. Coordinates in reports refer to original canvas pixels, including crops.
Review and checkpoint management leave artwork, revision, recovery and undo/redo
unchanged. A valid failed rule returns `passed:false` in a successful response;
invalid parameters/IDs and exceeded resource budgets are tool errors.

## Review sheets

```json
{"project_id":"project-1","frame_id":"frame-1","display_scale":2,"enlarged_scale":8,"views":["color","silhouette","grayscale"],"backgrounds":["#FFFFFFFF","#202020FF"]}
```

Call `get_review_sheet`. Optional `layer_id` isolates a layer, forcing it visible
for this preview only; optional `crop` is `{x,y,width,height}` wholly within the
canvas. Scales are positive integers; defaults are display 1 and enlarged 8.
Native 1x is always included. Duplicate scales share panels with multiple
`scale_roles`. Choose 1..3 distinct representations and 1..4 opaque RGBA
backgrounds. Defaults are all representations on white and dark gray.

Silhouette uses black RGB and preserves alpha. Grayscale preserves alpha and uses
integer Rec.709 luminance weights 2126/7152/722, denominator 10000, nearest rounding.
All enlargement is nearest-neighbor. Images have neutral representation/scale/
background labels; intended subjects and semantic layer/frame names are absent.
`panels[].image_bounds` maps sheet positions to the reported original `source`.

## Independent before/after checkpoints

Call `manage_checkpoint` with one of:

```json
{"project_id":"project-1","action":"create","expected_revision":4,"label":"readable blockout"}
```

```json
{"project_id":"project-1","action":"list"}
```

```json
{"project_id":"project-1","action":"delete","expected_revision":7,"checkpoint_id":"checkpoint-1"}
```

Creation returns a never-reused ID and creation revision. Labels are 1..80
characters on one line. Checkpoints survive editing, undo/redo and `save_project`,
independently of undo eviction. They end on `finish_project` or server exit.
They are **session-local**, not embedded in `.pixloom`; export another `.pixloom`
copy if a durable baseline is needed. At most 16 snapshots/64 MiB total are kept.
Capacity failures never silently evict an earlier snapshot or consume an ID.

Compare with `compare_checkpoint`:

```json
{"project_id":"project-1","checkpoint_id":"checkpoint-1","frame_ids":["frame-1"],"include_images":true,"scale":8}
```

Optional `layer_ids` restricts compared layers; optional `crop` lies within the
union of both canvases. Defaults cover all frames/layers, include images and
use scale 8. Set `include_images:false` for metadata-only comparisons.

Stable IDs align reordered frames/layers. Reports include structural differences,
added/removed targets, per-cel index/resolved-color counts and changed bounds,
and rendered pixel differences. A palette-only color change counts as resolved
change; duplicate colors at different indices remain separate index changes.
Structural array changes include complete change totals and up to 256 before/after
entry examples. Canvas resizing is explicit; out-of-canvas pixels are absent.

Rendered categories are disjoint: additions, removals, repainting, and alpha-only
changes. Two fully transparent rendered pixels are visually equal. Changed pixels
are highlighted pink; unchanged pixels are faint. Before/current/difference panels
are provided for selected frames. Neither fewer differences nor more differences
means progress by itself.

## Sprite measurements

```json
{"project_id":"project-1","frame_id":"frame-1","alpha_threshold":1,"connectivity":8,"include_annotation":true}
```

Call `analyze_sprite`; optional layer/crop use the same semantics as review sheets.
Alpha threshold is 1..255; connectivity is 4 or 8. The report provides occupied
bounds, padding within the chosen source region, fill ratio, occupied/unoccupied
counts, centroid, rendered color inventory, and connected-component areas/bounds.
An empty region has null bounds, padding and centroid. Coordinates are canvas-relative.

Raw inventory counts exact palette indices, including duplicate RGBA entries.
With no layer selected it includes hidden layers too; rendered inventory reflects
visible composition. Rendered colors count all nonzero alpha, independently of the
occupancy threshold. `transparent_pixels` counts alpha zero; `unoccupied_pixels`
uses the supplied threshold. Partial-opacity composition can produce colors outside
the authoring palette.

One-pixel components are inspection candidates, not confirmed mistakes. Optional
annotated PNG evidence uses numbered component bounds and matching JSON IDs.
`annotation_scale` records integer enlargement (up to 8x, reduced to fit the
image budget); evidence coordinates remain in source pixels. Inspector labels
use screen-sized text so they do not become giant glyphs in small sprites.
At most 256 components/color records per category are returned. Totals remain
complete; `truncation` identifies omitted records. Crop/isolate to investigate
an omitted area. Component IDs identify this report, not persistent artwork objects.

## Caller-supplied validation

```json
{"project_id":"project-1","rules":[
  {"type":"canvas_size","width":32,"height":32},
  {"type":"frame_count","min":1,"max":8},
  {"type":"frame_duration","min_ms":50,"max_ms":250},
  {"type":"allowed_palette_indices","indices":[0,1,2],"scope":{"frame_ids":["frame-1"],"layer_ids":["ink"]}},
  {"type":"max_rendered_colors","max_colors":12},
  {"type":"binary_alpha"},
  {"type":"protected_region","checkpoint_id":"checkpoint-1","frame_ids":["frame-1"],"region":{"x":8,"y":4,"width":8,"height":8}}
]}
```

Call `validate_artwork` with 1..64 rules. Count/duration ranges are inclusive;
durations must be positive and finite. `frame_duration` optionally takes
`frame_ids`. Color/alpha rules optionally take `scope:{frame_ids,layer_ids,crop}`;
arrays must be nonempty, unique and refer to actual targets. A single explicitly
scoped rendered layer is forced visible; unspecified composition retains saved
visibility. Allowed indices are raw cel checks. `max_rendered_colors` counts the
union of nontransparent RGBA across selected frames. Empty artwork has zero colors.

Protected regions compare composited pixels with their checkpoint by default;
optional `layer_id` isolates a layer. Palette and opacity changes are detected.
Regions must fit the baseline. A current canvas that cannot cover the protected
region, or a target removed from one side, fails protection. Unknown checkpoints,
IDs missing on both sides and malformed ranges are argument errors.

The report provides per-rule expected/observed evidence, total violations and at
most 256 example coordinates/targets per rule. These rules have no built-in style
preferences. They are supplied per call and do not alter the project schema.

## Animation audits

```json
{"project_id":"project-1","frame_ids":["idle-1","idle-2","idle-3"],"include_loop_transition":true,"max_centroid_displacement":2,"max_area_ratio":1.5,"scale":2}
```

Call `audit_animation`. Ordered unique `frame_ids` default to the entire timeline;
optional `layer_id`/`crop` select visible evidence. `alpha_threshold` defaults to 1,
`scale` to 1, and `include_loop_transition` to false. Thresholds are optional:
centroid displacement must be finite/nonnegative, area ratio finite and >=1.

The report includes exact durations and total, distinct rendered poses, consecutive
identical holds and combined durations, per-frame bounds/areas/centroids, consecutive
pixel differences/ratios/bounds and a neutral-labeled contact sheet. Subsets use the
requested order; their transitions may skip intervening source frames. Two empty
frames have area ratio 1; an empty/nonempty pair has null ratio and an explicit
empty-transition observation. No nonfinite JSON values are produced.

Including the closing transition measures last-to-first; a good loop need not have
identical endpoint frames. Edge contacts, holds and centroid movement are facts.
Only explicitly supplied thresholds produce warnings. Nothing is automatically
realigned, retimed or redrawn. Exact distinctness uses compressed image evidence,
with a separate 64-MiB budget; reduce frame selection if exceeded.

## Inspector integration

The existing menu has a **Review** submenu for Color/Silhouette/Grayscale,
Component bounds, Analyze current frame, Review sheet and Audit animation.
Standalone inspector use can run the latter three locally without MCP.

MCP reviews open an **Art review** panel containing summaries, PNG evidence and
expandable JSON. Checkpoint reviews show before/current/difference sheets; their
checkpoint ID appears in the menu. **Return to live artwork** hides the review
and restores the normal representation. Animation transitions can be selected
for the existing frame-comparison view when inspecting manually.

While Follow AI is enabled, incoming reviews select their live frame/layer and
representation. While it is disabled they do not replace the manual view;
re-enabling it adopts the newest request. Ordinary `inspect_project` returns to
live inspection. Reports whose source differs from current artwork are marked
**Stale report** and their component overlays are disabled. Reports remain evidence
of their recorded revision, not measurements of later artwork. Native source-load
errors still appear through the inspector's existing error display.

Large review images are fitted to the graphics device's texture limit with
nearest-neighbor scaling and preserved aspect ratio. The full PNG returned to
MCP retains its original dimensions; panel coordinates refer to that full image.

## Resource limits

Images/sheets, including labels/padding, are limited to 16 million pixels.
Multi-frame work permits at most 64 selected frames and 64 million source pixel
visits per side; layer/canvas-union scans count toward work. Cropping does not
remove the cost of loading/compositing full source canvases. Split requests or
select fewer targets to reduce work; there is no silent partial audit.

Managed review IPC is capped at 64 MiB serialized, including at most 40 MiB of PNG
bytes before base64 encoding. Oversized payloads fail with reduction guidance.
Review reports and images do not become project content or affect normal exports.
For the future critic's intended use, see [critic workflow](critic-workflow.md).
