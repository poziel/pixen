# Pixen evidence-based art review

Date: 2026-10-18
Status: Implemented and verified locally; see the implementation plan for evidence.

User scope correction: do not create or modify skills or critic sub-agents. Write
docs/critic-workflow.md as future design guidance only; implement MCP APIs and inspector integration.

## Purpose and accepted direction

Implement the five capabilities discussed with the user: review views, before/after
checkpoints, sprite measurements, project-specific validation, and animation audits.
They support a pixel-art skill and an optional independent critic that describe,
review and correct artwork using actual images and reproducible measurements.

Success means an agent can identify a visible issue, locate its evidence, make a
bounded edit, and compare the result with the original brief and baseline. File
validity, visual measurements, recognizability and subjective taste remain distinct.
The server does not assign a universal artistic-quality score or infer improvement
from a lower color count, fewer isolated pixels, symmetry, or smaller differences.

This design extends the current shared Rust editing/rendering library and the
existing dedicated pixen-mcp executable. Preserve desktop/CLI/MCP separation,
all existing tools, exact palette-index semantics, native compatibility, and the
current user work in progress. No external model calls, hosted services, automatic
sub-agent spawning, paid API dependencies, or automatic corrective edits are added.

## Approach and alternatives

Recommended: deterministic review primitives in the shared library, typed MCP
wrappers, matching desktop views where meaningful, and a documented critic loop
in the Pixen skill. Callers supply task-specific rules and interpretation.

An all-in-one assess-art endpoint would be simpler to call, but would mix opinions
with measurements and conceal which check matters. Hosting a critic model inside
the server would introduce model selection, authentication, cost and latency that
belong to the client/skill orchestration. Neither alternative is selected.

## Existing implementation boundaries

- src/session.rs owns one active authoring document, monotonically increasing
  revisions, recovery files, and bounded undo/redo. Undo snapshots may be evicted.
- src/inspection.rs provides the shared composite/onion/comparison render paths.
- src/mcp.rs wraps Session and forwards inspection state to the managed viewer.
- src/viewer.rs provides manual view controls and Follow AI synchronization.
- src/preview.rs provides bounded nearest-neighbor image/contact-sheet rendering.
- skills/pixen/SKILL.md describes tool usage. Pixel-art craft guidance remains
  tool-independent and is not rewritten or copied into the server.

New review rendering, measurement, comparison, rule and animation logic should be
focused shared modules rather than substantial algorithms inside mcp.rs/viewer.rs.
Session owns baseline lifecycle; the viewer does not own or mutate artwork.

## API conventions

Six new tools implement the five capabilities. Names below are the selected API
proposal. All artwork targets use stable frame/layer IDs. Read-only inspection may
update the managed viewer only while Follow AI is on, as existing inspection does.

Reports include report_version:1, project_id, inspected_revision, explicit frame/
layer/region scope, measurement parameters, findings and truncation metadata.
Coordinates always refer to original canvas pixels, not scaled image positions.
Results distinguish observed facts, caller-rule failures and heuristic warnings.

Invalid IDs/parameters and exceeded budgets are MCP tool errors. A valid check that
finds a rule violation is a successful tool response containing passed:false.
Findings use stable codes, severity, evidence, bounds/coordinates, and explanatory
text; they never present a heuristic suspicion as a confirmed artistic defect.

Read-only calls and checkpoint management do not change document revision,
recovery, artwork, undo or redo. Checkpoint mutation uses expected_revision to
ensure the captured source is the document the caller intended.

## 1. Review views: get_review_sheet

Inputs: project_id, frame_id, optional layer_id and crop, display_scale (positive
integer, default 1), enlarged_scale (positive integer, default 8), requested views
(selected from color/silhouette/grayscale), optional opaque RGBA backgrounds
(maximum four; default white and dark gray).

Return a labeled PNG sheet of requested representations at native, display and
enlarged scales, plus exact panel/source bounds in JSON. Identical scales need
only one panel, identified with all corresponding scale labels. Use nearest-
neighbor scaling throughout. Layer isolation forces only the requested layer
visible in the review, preserving saved visibility/opacity and all other artwork.

Silhouette uses black RGB and preserves source alpha; grayscale uses a documented
standard luminance conversion and preserves alpha. Background compositing is
explicit and presentation-only. Labels and backgrounds never affect measurements.
Panel labels identify only representation, scale and neutral frame ordinals; they
do not expose semantic layer/frame names. No reference image or intended semantic
description is embedded in the pixels.

Expose silhouette/grayscale as mutually exclusive representation choices in the
inspector menu, with color as the normal representation. Their alpha/background
behavior matches MCP renders. Existing onion-skin/frame-comparison modes remain
available; a review sheet selects ordinary frame review with its first requested
representation while following. The multi-panel sheet itself is returned to the
AI; the desktop need not become a sheet-layout editor. Manual controls remain
independent while Follow AI is off.

## 2. Checkpoints: manage_checkpoint and compare_checkpoint

manage_checkpoint is a typed action tool:

- create: project_id, expected_revision, nonblank label (1..80 characters).
  Capture the complete authoring document, assign a never-reused checkpoint_id,
  and return its creation revision and byte size. Duplicate labels are allowed;
  checkpoint IDs identify snapshots.
- list: project_id. Return ordered IDs, labels, creation revisions, size and
  available checkpoint budget.
- delete: project_id, expected_revision, checkpoint_id. Explicitly release one baseline. Unknown
  checkpoints are errors; deletion never modifies artwork/history.

Checkpoints are immutable, session-local, and independent of undo/redo. They survive
undo, redo, successful edits and save_project, and are released on finish/server
exit. No silent eviction. Limit to 16 snapshots and 64 MiB total serialized bytes
per project, separately from the existing undo budget. Failed creation does not
reserve an ID or discard another checkpoint. Persistence inside project containers
is deferred; export/save a separate .pixloom copy when a durable source is needed.

compare_checkpoint inputs: project_id, checkpoint_id, optional frame_ids and
layer_ids, optional crop, optional include_images (default true), scale (default 8).
Default comparison covers all frames/layers. Explicit IDs must exist in at least
one side; report added/removed IDs instead of silently ignoring missing targets.

Return structural differences (canvas, layers/order/properties, frames/order/
timing, palette and tags), exact per-cel changed bounds/counts, and rendered visual
differences for matching frames. Stable IDs align frames/layers across reorder.
Exact cel comparison considers both stored palette index and its resolved RGBA;
changing a palette entry counts even if stored indices are unchanged. Report raw
index changes separately from rendered changes. Compare different canvases over
the union rectangle with out-of-canvas areas treated as absent, and report size
changes explicitly.

Classify rendered pixels as silhouette additions, silhouette removals, repainting
where both sides have nonzero alpha, and alpha-only changes. Categories are
disjoint; all rendered changes count once. Two fully transparent composited pixels
are visually equal. Return before/current/difference panels for requested frames,
with coordinates and revisions. Never claim that fewer changes mean better art.

The desktop checkpoint-comparison view is inspection-only, receives the chosen
baseline/current images through bounded managed-view IPC, and shares rendering
with MCP. Its menu identifies the active checkpoint and offers return to live
artwork. While Follow AI is off, incoming baseline review requests do not replace
the manual view. A later normal inspection request restores live frame viewing.

## 3. Sprite measurements: analyze_sprite

Inputs: project_id, frame_id, optional layer_id and crop, alpha_threshold (1..255,
default 1), connectivity (4 or 8, default 8), include_annotation (default true).

Return occupied bounds (null for empty), per-side transparent padding (null for an
empty region), occupied/transparent counts, fill ratio, and occupied-pixel centroid
(null when empty). Coordinates are canvas-relative; padding is relative to scope.

Return two color inventories with distinct semantics: exact stored palette-index
usage for explicitly targeted cels, and rendered RGBA usage for the composed
image. Duplicate palette entries retain separate identity. Transparent entries
are counted separately; compositing may generate RGBA colors outside the palette.

Detect occupied connected components under the specified alpha/connectivity rule.
Return component IDs, areas and bounds, and explicitly flag one-pixel components
as candidates for inspection. Do not auto-delete or call them mistakes. The
optional image labels component locations with a matching ID-to-bounds map. Expose
an inspector overlay for these labels/bounds and a short measurement summary;
users can disable the overlay locally. Drawing pixels and exports stay unchanged.

## 4. Task-specific checks: validate_artwork

Inputs: project_id and a nonempty typed rules array (maximum 64). Rules are supplied
by the caller; no style is silently imposed or inferred. The report returns an
overall passed flag and one result per rule with expected/observed evidence.

Initial rules:

- canvas_size: exact width/height.
- frame_count: inclusive minimum/maximum.
- frame_duration: inclusive minimum/maximum milliseconds, optional frame IDs.
- allowed_palette_indices: only selected indices may occur in explicitly scoped
  raw frame/layer cels; palette slots themselves are never renumbered.
- max_rendered_colors: count distinct nontransparent rendered RGBA within scope.
- binary_alpha: composed or explicit-layer alpha must be 0 or 255.
- protected_region: reference checkpoint_id, explicit rectangle and frame IDs,
  optional layer_id; verify rendered pixels remain identical. Missing baseline/
  current frame or layer is a failed rule. Use composite pixels by default, or an
  isolated layer with layer_id. This catches visible palette/opacity changes too.

Each rule uses explicit applicable scopes. Reject conflicting/unsupported fields,
invalid bounds, inverted ranges, unknown indices or nonexistent checkpoints as
argument errors. Protected-region rectangles are checked against the baseline;
a resized current canvas failing to cover them is a failed protection rule.
A structurally added/removed object is evidence, not a server
crash. An all-transparent scope has zero nontransparent colors. Return total
violation counts plus a bounded coordinate sample and bounds, not millions of
individual entries. No automatic repair or generated aesthetic rating.

Rules are supplied per call in this version. Persistent art-direction profiles and
project-schema changes are deferred, keeping .pixloom/native formats unaffected.
The inspector may show the last rule report as read-only status; reports from an
older revision must be labeled stale after an artwork change.

## 5. Motion review: audit_animation

Inputs: project_id, optional ordered unique frame_ids (default entire timeline),
optional layer_id/crop, alpha_threshold (default 1), include_loop_transition
(default false), optional warning thresholds for centroid displacement and
frame-to-frame occupied-area ratio.

Return actual frame durations, total duration, distinct rendered-pose count,
consecutive identical holds and their combined durations, per-frame occupied
bounds/area/centroid, and consecutive-pair changed pixels/ratio/bounds. Report
occupied canvas-edge contacts as observations, not proof of clipping.

With include_loop_transition:true, additionally report the last-to-first pair;
a good loop does not require identical endpoint frames. Analyze requested frames
in their declared order, and state that subset transitions may skip source frames.
An empty frame has no centroid. Holds, centroid changes and area changes are facts;
only caller-selected thresholds produce warnings, and none proves bad animation.
Do not align or translate frames automatically to hide motion differences.

Return a labeled contact sheet with frame/timing/measurement metadata, using the
existing bounded sheet renderer. The inspector can display the report alongside
playback and select a reported frame pair for existing comparison/onion views.
It must preserve real frame timing and saved document data.

## Resource limits and freshness

- All image outputs retain the existing 16-million-pixel limit; review sheets
  include every panel and label in that budget. Reject oversized requests with
  guidance to reduce scales/views/frame selection, never silently downscale.
- Multi-frame measurement/comparison requests allow at most 64 selected frames
  and at most 64 million source pixel visits per side; stream rendered frames
  rather than retaining a full timeline of RGBA buffers. Requests over limits
  fail clearly with subset guidance; no silent incomplete audit.
- Return at most 256 component records/color inventory entries per category and
  256 example coordinates per finding. Totals remain complete; responses expose
  truncation flags and total counts, with crop/layer guidance for closer review.
  Component labels use the same returned subset, deterministically ordered.
- Baseline images/annotations use bounded payloads. Reopening a viewer or toggling
  Follow AI cannot send an unbounded full authoring document through viewer IPC.
- Every result names its inspected revision; older reports/images are not presented
  as measurements of later artwork. Read-only calls never modify revision/history.

## Skill and independent critic workflow

Document this workflow in the repository Pixen skill without modifying the
user's external pixel-art skill:

1. Define intent, display size, permitted palette/alpha, protected areas, and
   animation intent. Convert objective constraints into validation rules.
2. Draw the initial blockout, create a checkpoint, and request review views.
3. If the client supports an independent critic, provide only the rendered views
   and viewing-scale information for the initial recognition pass. Exclude the
   intended subject, descriptive filenames, activity text and suggestive labels
   from that critic prompt. Return a literal description, uncertainty and locations.
4. Compare the critic's observation with the brief. Description mismatch is evidence
   to investigate, not conclusive proof of poor art: ambiguity or critic error is
   possible. Reveal the brief for the subsequent task-specific critique.
5. Pick one concrete issue, inspect its layer/region/measurements, make a bounded
   correction, and compare_checkpoint. Re-run rules and relevant visual views.
6. Judge progress against declared goals. For an optional second blind review,
   randomize before/after order and conceal revision labels in the critic's input.
   Keep revisions in the orchestrator's record. A critic preference is subjective;
   report disagreements with objective checks rather than merging them into a score.
7. Stop after explicit success criteria are met or after a small configured review
   budget (recommended maximum three correction rounds); surface unresolved issues
   instead of repeatedly redrawing. Preserve the chosen baseline and editable art.

The MCP server exposes evidence. The client/skill controls whether a critic runs,
which model it uses, and its cost/time budget. No critic agent is launched merely
because an MCP measurement or preview tool is called.

## Verification and acceptance

Use test-driven implementation in focused stages, preserving all current tests.

- Golden pixel tests: representations, alpha/background compositing, source-space
  panel coordinates, scale limits, and hidden-layer inspection.
- Checkpoint lifecycle: independent undo/redo, stale revision rejection, capacity
  errors without eviction, deletion, release and no document/recovery mutation.
- Comparison fixtures: palette-only and index-only changes, additions/removals,
  opacity/reorder, changed canvas sizes, stable frame IDs and disjoint categories.
- Measurements: empty/opaque/partial-alpha images, duplicate indices, 4/8-connected
  diagonals, deterministic truncation/labels and exact component bounds.
- Rule checks: positive/negative cases, malformed rules, protected palette/opacity
  changes, missing protected targets, and honest pass/fail/tool-error separation.
- Animation: intentional holds, empty frames, duration sums, monotonic translations,
  loop transitions, subsets, thresholds and resource-budget failures.
- MCP protocol tests: actual image content, schema validation, report revisions,
  normal error handling, and unchanged artwork/undo state after analysis.
- Headless egui tests: manual toggles, matching render pixels, Follow AI on/off,
  stale report indication and return from checkpoint views to live artwork.
- Run relevant suites, Clippy, formatting/diff checks and the release build.
  Report native desktop smoke tests separately if no interactive test is run.

Implementation order: shared review rendering; checkpoint storage/comparison;
measurements; rule validation; animation audit; inspector/skill integration and
end-to-end verification. No push, publishing, user PATH/configuration changes or
external skill-file changes are part of this work.
