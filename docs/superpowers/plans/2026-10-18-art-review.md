# Pixloom Art Review Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Implement the five approved evidence-based art-review capabilities through six MCP tools and matching inspector features.

**Architecture:** Add focused shared Rust modules for review rendering, checkpoints/comparison, measurements, rules and animation. Session owns immutable baselines; MCP remains a typed transport adapter. The desktop consumes the same rendering/report models, preserving manual controls and Follow AI behavior.

**Tech Stack:** Existing Rust, serde/serde_json, schemars, image, ab_glyph, rmcp and egui dependencies; no added runtime dependencies.

**Spec:** [Approved art-review design](../specs/2026-10-18-art-review-design.md).

## Global Constraints

- Preserve all existing tools, exact palette-index semantics, native compatibility, dedicated desktop/CLI/MCP entry points and existing user work in progress.
- Read-only calls and checkpoint management do not change document revision, recovery, artwork, undo or redo.
- No external model calls, hosted services, automatic sub-agent spawning, paid API dependencies, or automatic corrective edits are added.
- Checkpoints are immutable, session-local, independent of undo/redo: 16 snapshots and 64 MiB total serialized bytes per project; no silent eviction.
- Image outputs: 16-million-pixel limit including every panel and label; no silent downscaling.
- Multi-frame work: at most 64 selected frames and 64 million source pixel visits per side; stream RGBA frames.
- Reports: report_version:1, inspected revision, explicit scope, full totals, at most 256 components/colors per category and 256 coordinate examples per finding, with truncation flags.
- Validation accepts 1..64 caller-supplied typed rules. Valid failures return passed:false; malformed inputs/budget violations are tool errors.
- Preserve Rust 1.89 minimum and current platform support; external pixel-art skills, PATH and MCP client configurations are outside scope.
- Leave changes uncommitted unless the user explicitly requests commits; do not push or publish.

## Review Focus

1. Palette-only edits must affect resolved-color/protected-region comparisons even when stored indices remain identical (Tasks 2 and 4).
2. An empty or partial-alpha sprite must yield honest null geometry, preserved alpha and threshold-dependent components rather than divisions by zero or fabricated bounds (Tasks 1, 3 and 5).
3. Removed/reordered frames/layers and resized canvases must compare by stable IDs, classify missing protected targets as failures and preserve all meaningful differences (Tasks 2 and 4).
4. Requests that would produce enormous reports/images or overflow dimensions must fail before large allocations; truncated evidence must retain complete totals (Tasks 1, 3, 5 and 6).
5. Late/stale review messages, source reloads and Follow AI toggles must not replace manual viewing unexpectedly or present old reports as current artwork (Task 6).

## File map and shared interfaces

Create `src/review/mod.rs` for public requests/results, scopes and budget validation.
Create `src/review/render.rs`, `checkpoints.rs`, `compare.rs`, `measure.rs`,
`rules.rs`, `animation.rs` for the corresponding pure/shared responsibilities.
Create `src/viewer_review.rs` for desktop review state, report UI and IPC payloads.
Modify `src/lib.rs` to expose review; `src/session.rs` for state access/lifecycle;
`src/inspection.rs` for reusable representation transforms; `src/mcp.rs` for wrappers;
`src/viewer.rs` for delegation into viewer_review. Avoid moving unrelated code.

Shared types in `review/mod.rs`:

- `Representation`: serde snake_case enum `Color`, `Silhouette`, `Grayscale`.
- `Scope { frame_ids: Option<Vec<String>>, layer_ids: Option<Vec<String>>, crop: Option<Crop> }` for multi-target requests. Single-frame requests carry explicit frame_id and optional layer_id/crop directly.
- `ReviewImage { role: String, png: Vec<u8> }`; image role never describes intended semantic content.
- `ReviewResult { report: serde_json::Value, images: Vec<ReviewImage> }` follows the repository's current JSON report conventions.
- `ReviewStamp { project_id: String, inspected_revision: u64 }` supplies report identity, never a model-invented revision.
- `validate_scope(doc: &Document, scope: &Scope) -> Result<ResolvedScope>` resolves stable IDs and checks unique selections, bounds and pixel budgets. `ResolvedScope` contains ordered frame/layer indices and a validated source Crop.
- `stamp(result: &mut ReviewResult, stamp: &ReviewStamp)` inserts report_version:1 and identity, retaining the report's typed scope/parameters/findings/truncation values.

Each request is Deserialize + JsonSchema with unknown fields denied and the exact
fields/defaults in the spec. Do not introduce a second document or index model.

### Task 1: Shared review representations and review sheet

**Files:** Create review/mod.rs and review/render.rs; modify inspection.rs/lib.rs; create tests/review_render.rs.

**Interfaces:**
- `transform_rgba(rgba: &[u8], representation: Representation) -> Vec<u8>` in inspection.rs; grayscale uses integer Rec.709 coefficients 2126/7152/722 with denominator 10000 and nearest rounding, preserving alpha.
- `review::render::sheet(doc: &Document, request: &ReviewSheetRequest) -> Result<ReviewResult>`.
- `ReviewSheetRequest`: frame_id, layer_id, crop, display_scale=1, enlarged_scale=8, views default `[color,silhouette,grayscale]`, backgrounds default white/#202020FF. Reject empty/duplicate views and more than four or nonopaque backgrounds.
- `review::render::frame(doc: &Document, frame_id: &str, layer_id: Option<&str>, crop: Option<Crop>) -> Result<ScopedRgba>`; ScopedRgba holds validated source Crop, width/height and pixels. Reuse this in Tasks 3–5; visibility is forced on only for isolated layer rendering.

- [x] Write golden tests: silhouette RGB is black with unchanged alpha; pure red grayscale becomes 54; crop metadata remains canvas-relative; duplicate scales share panels; native/display/enlarged labels stay neutral; hidden-layer isolation leaves saved visibility unchanged.
- [x] Add boundary tests: zero/u32::MAX scales, alpha backgrounds, empty view list, four versus five backgrounds, panel-label pixel overhead exceeding 16-million budget. Assert errors before output allocation.
- [x] Run `cargo test --locked --test review_render`; confirm expected missing-interface failure.
- [x] Implement shared types, preflight sheet layout and representation transforms. Reuse existing font/rendering primitives; keep labels outside measured source pixels.
- [x] Run the same tests; require exact pixels/metadata and zero failures.

### Task 2: Immutable checkpoint lifecycle and comparison

**Files:** Create review/checkpoints.rs and review/compare.rs; modify session.rs; create tests/review_checkpoints.rs.

**Interfaces:**
- `CheckpointStore::default()`, `create(doc: &Document, revision: u64, label: &str) -> Result<Value>`, `list() -> Value`, `delete(id: &str) -> Result<Value>`, `document(id: &str) -> Result<(Document,u64)>`. Store immutable serialized document bytes plus label/revision in creation order; use never-reused integer IDs.
- Add `checkpoints: CheckpointStore` to Project and initialize it on create/open. Keep it outside undo snapshots; dropping Project releases it.
- `CheckpointAction`: internally tagged action=create/list/delete; create/delete require expected_revision, create requires label, delete requires checkpoint_id. Validate label trims to 1..80 characters and has no control characters.
- `Session::manage_checkpoint(&mut self, id: &str, action: CheckpointAction) -> Result<Value>` checks revision before create/delete.
- `compare::documents(before: &Document, after: &Document, request: &CompareCheckpointRequest) -> Result<ReviewResult>`; request has checkpoint_id, optional frame_ids/layer_ids/crop, include_images=true, scale=8.
- `compare::pixels(before: &[u8], after: &[u8], width: u32) -> PixelDiff` implements shared disjoint categories, changed totals/bounds and bounded examples for Tasks 4–5. PixelDiff is Serialize with added/removed/repainted/alpha_only counts and changed_pixels; transparent-to-transparent RGBA differences are visually equal.
- `Session::compare_checkpoint(&self, id: &str, request: &CompareCheckpointRequest) -> Result<ReviewResult>` loads baseline and stamps both revisions.

- [x] Write lifecycle tests: checkpoints survive edits, undo/redo and save; stale create/delete fails; the 17th entry and 64-MiB overflow fail without eviction or reserving IDs; finish releases baselines. Compare recovery bytes/revision/history before/after management.
- [x] Write comparison tests: palette-only color change, duplicate-index-only change, opacity/reorder, added/removed frame/layer, reordered stable IDs, union-canvas comparison, invalid selected ID and empty-to-empty images. Assert each changed visual pixel belongs to exactly one category.
- [x] Run `cargo test --locked --test review_checkpoints` and confirm failures.
- [x] Implement bounded storage and structural/raw/composited comparison. For both-nontransparent pixels classify unchanged RGB + changed alpha as alpha_only; RGB changes as repainting. Alpha threshold for these categories is nonzero alpha, independent of measurement thresholds.
- [x] Generate before/current/difference panels via Task 1 layout helpers; preflight all selected output frames and union crops. Reject unknown IDs on both sides; report IDs missing on one side as structural differences.
- [x] Run lifecycle/comparison tests and existing `cargo test --locked --test editing --test mcp_extensions`.

### Task 3: Sprite measurements and annotated component evidence

**Files:** Create review/measure.rs; create tests/review_measure.rs.

**Interfaces:**
- `AnalyzeSpriteRequest`: frame_id, optional layer_id/crop, alpha_threshold=1 (1..255), connectivity=8 (4 or 8), include_annotation=true.
- `measure::pixels(scope: &ScopedRgba, alpha_threshold: u8, connectivity: u8) -> Result<Value>` returns exact geometry, rendered color counts, component totals/records, findings and truncation.
- `measure::sprite(doc: &Document, request: &AnalyzeSpriteRequest) -> Result<ReviewResult>` adds raw palette-index counts from each selected frame/layer cel, separately from composited RGBA inventory. With layer_id omitted, raw inventory covers all document layers for the selected frame, explicitly including hidden layers; rendered inventory uses normal visible composition.
- `measure::annotate(scope: &ScopedRgba, report: &Value) -> Result<ReviewImage>` draws neutral component IDs and bounds for the returned deterministic subset only.

- [x] Write tests for empty geometry/null centroid/padding, partial-alpha thresholds, crop-relative padding and canvas-space centroid, diagonally touching pixels under 4/8 connectivity, and identical RGBA stored at distinct indices.
- [x] Add >256 components/colors tests: totals remain complete, records/labels are capped, truncation is explicit and order is stable (components scan top-to-bottom/left-to-right; inventories sort by palette index or RGBA).
- [x] Run `cargo test --locked --test review_measure` and observe failures.
- [x] Implement iterative bounded flood fill (no recursive stack growth), coordinate conversion, histograms and annotations. One-pixel components are neutral inspection candidates, never confirmed mistakes.
- [x] Run the measurement tests and Task 1 render suite.

### Task 4: Caller-supplied artwork rules

**Files:** Create review/rules.rs; modify session.rs; create tests/review_rules.rs.

**Interfaces:**
- `ArtRule`: internally tagged type=canvas_size/frame_count/frame_duration/allowed_palette_indices/max_rendered_colors/binary_alpha/protected_region, with the exact scopes and bounds in the spec; minimum/maximum are inclusive, duration values finite and positive.
- `ValidateArtworkRequest { rules: Vec<ArtRule> }`.
- `rules::validate(doc: &Document, checkpoints: &CheckpointStore, request: &ValidateArtworkRequest) -> Result<ReviewResult>`; report includes passed and ordered per-rule expected/observed/results/finding evidence.
- `Session::validate_artwork(&self, id: &str, request: &ValidateArtworkRequest) -> Result<ReviewResult>` stamps current revision.

- [x] Write positive and negative fixtures for all seven rules. Assert a violated valid rule returns Ok with passed:false; malformed ranges/IDs/indices, unsupported fields, empty or >64 rules and nonexistent checkpoints return Err.
- [x] Test protected regions after palette-only/opacity edits, missing current frame/layer and smaller current canvas. Bounds must fit the baseline; absence on the current side is failed protection. Assert an unchanged region passes despite unrelated edits.
- [x] Test 256 example-coordinate cap, complete counts, zero colors in transparent art, and hidden/isolated layer semantics using Task 3 inventories.
- [x] Run `cargo test --locked --test review_rules` and observe failures.
- [x] Implement prevalidation before scanning, then complete checks using shared scoped renders/comparison. Protected frame/layer IDs are allowed to be missing on one side and count as violations rather than argument errors; IDs missing on both sides are invalid input.
- [x] Run rule, checkpoint and measurement suites.

### Task 5: Animation audit with meaningful transition evidence

**Files:** Create review/animation.rs; create tests/review_animation.rs.

**Interfaces:**
- `AuditAnimationRequest`: optional ordered unique frame_ids, optional layer_id/crop, alpha_threshold=1, include_loop_transition=false, optional finite nonnegative max_centroid_displacement and max_area_ratio (ratio must be >=1).
- `animation::audit(doc: &Document, request: &AuditAnimationRequest) -> Result<ReviewResult>` streams scoped frame renders and produces frame measurements, transition diffs, hold runs, duration sums, distinct-pose count, optional threshold warnings and a labeled contact sheet.
- Distinctness compares exact rendered scoped pixels (verify equal hashes with actual image equality if hashing is used). Consecutive hold durations include all frames in each run. Area ratio is max(a,b)/min(a,b); two empty frames yield 1, exactly one empty yields null plus an explicit empty-transition observation, avoiding nonfinite JSON.

- [x] Write tests for unequal-duration holds, one static frame, empty frames, translation centroid/edge contacts, requested subset order and last-to-first inclusion. Assert mismatched loop endpoints do not automatically fail.
- [x] Add alpha-threshold tests, invalid/duplicate frame IDs, >64 frames, pixel-work budget and image-size overflow. Threshold-free reports contain observations without aesthetic warnings.
- [x] Run `cargo test --locked --test review_animation` and observe failures.
- [x] Implement streaming measurements/differences and bounded sheet output with neutral frame labels; reuse Task 2 comparison and Task 3 geometry. Preserve actual durations and include declared subset/loop parameters.
- [x] Run animation, render and measurement suites.

### Task 6: Inspector integration and typed MCP transport

**Files:** Create viewer_review.rs; modify viewer.rs/mcp.rs/session.rs; extend tests/mcp_protocol.rs; create tests/review_protocol.rs if splitting the existing Client helper makes test ownership clearer.

**Interfaces:**
- Session methods `review_sheet(id,&ReviewSheetRequest)`, `analyze_sprite(id,&AnalyzeSpriteRequest)`, `audit_animation(id,&AuditAnimationRequest)` return Result<ReviewResult>; all stamp actual current revision. Earlier tasks already define checkpoint/validation methods.
- Six tools: get_review_sheet, manage_checkpoint, compare_checkpoint, analyze_sprite, validate_artwork, audit_animation. Public wrappers accept project_id plus each task's typed request; no CLI MCP alias returns.
- `ViewerReview { project_id, inspected_revision, frame_id, kind, report, images }` is the serde IPC message; kind selects representation/checkpoint/measurement/validation/animation. Images use bounded PNG bytes encoded by transport, never authoring documents. Reject a serialized review message over 64 MiB with explicit reduction guidance before publishing it.
- `viewer_review::ReviewState` owns latest pending AI request, local view selection, representation/annotation toggles, displayed report and stale status. `receive`, `follow`, `menu`, `panel` and `render` methods let viewer.rs delegate without embedding review algorithms.
- Viewer freshness uses the recovery source's content identity (metadata alone is insufficient) captured when producing the report and rechecked after reload. Keep source identity out of critic images; report source revision when stale. A failed reload preserves the last valid source/report but indicates the source error.

- [x] Add protocol tests enumerating all six tools; assert actual PNG content, complete stamped JSON, malformed arguments as errors, valid failed rules as successful responses, and unchanged revision/history/recovery after analysis. Extend expected tools/list from 24 to 30.
- [x] Add headless egui tests for silhouette/grayscale/normal choices, annotation toggle, checkpoint identity and return-to-live menu, report-based pair selection, paused inspection and exact shared render pixels.
- [x] Test Follow AI off receiving several requests: manual view stays fixed; turning it on adopts the newest request. Test ordinary inspection after a checkpoint restores live view, then changes to artwork label previous measurements stale. Reopened viewers get the latest bounded review state.
- [x] Run focused protocol/viewer tests to observe failures before adding integration.
- [x] Wire wrappers through shared Session methods, reuse MCP image encoding, and add bounded review IPC. Honor source reload lag without silently selecting the wrong stable ID. A multi-frame report selects its first current frame for desktop display and keeps the whole report available.
- [x] Add inspector panel UI for measurement summaries, rule pass/fail and animation transitions; clicking a transition selects existing comparison/onion views. Keep controls available manually and displayed read-only while following.
- [x] Run `cargo test --locked --lib --test mcp_protocol --test mcp_extensions` plus the new protocol test file if created.

### Task 7: Critic design document, API documentation and complete verification

**Files:** Modify docs/mcp.md and README.md; add docs/art-review.md and docs/critic-workflow.md. Per the user scope correction, do not edit any skill or create a critic sub-agent.

**Interfaces:** Documentation uses the six final schemas from Task 6 and preserves normal CLI authoring plus .pixloom-first MCP persistence. No runtime critic agent is introduced.

- [x] Document a complete blockout/checkpoint/review/targeted-edit/compare/validate/audit workflow, with valid JSON calls and report coordinate/revision interpretation.
- [x] Add blind recognition and subsequent informed critique prompt templates. First-pass inputs omit intent, semantic filenames/activity text and suggestive labels; before/after preference can randomize order while the orchestrator retains identities. Specify maximum three correction rounds unless the caller chooses another explicit budget.
- [x] Explain ephemeral checkpoints, independent undo budgets, truncated evidence, exact-index versus rendered colors, loop limits, valid failed rules versus errors, stale reports and subjective critic disagreement. State that all thresholds are task-specific.
- [x] Run the documented calls through a real headless MCP end-to-end fixture with added/removed pixels, an intentional hold, a protected region and one negative validation result. Validate saved .pixloom/native exports still match artwork.
- [x] Run `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and `cargo build --locked --release`.
- [x] Review only task-owned changes against the initial dirty worktree, update this plan's completed checkboxes and the spec's implementation status. Report actual test totals, ignored native/desktop tests and release artifact paths; make no unperformed visual-test claim.

## Plan self-review

- All five approved capabilities map to Tasks 1–5 and shared inspector/MCP delivery in Task 6; critic integration and acceptance map to Task 7.
- The five Review Focus conditions each have explicit negative/boundary tests in their owner tasks.
- Shared request names, ReviewResult and Session methods are defined before transport integration. Task 1 frame/representation primitives are reused rather than forked.
- Checkpoint budgets are independent of undo; no new project/container schema or user-managed configuration is required.
- The user approved direct implementation with inspector integration and a critic design document only. Checkpoint 5278f2d was committed before product changes.

## Completion evidence

Baseline checkpoint: `5278f2d`, committed before implementation. All seven tasks
completed in the existing feature workspace. Skill and critic-agent implementation
were excluded per the user; docs/critic-workflow.md is design guidance only.

Full suite: 195 passed, 0 failed, 7 ignored. Two additional boundary cases were
then added and passed in the 14-test review_contract suite. Clippy, rustfmt, diff
checks and release build passed. A real release stdio smoke test exposed all 30
tools, generated review images, detected a one-pixel edit, returned an expected
protected-region failure, saved .pixloom and exported Aseprite with identical
rendered pixels. Native interactive window tests were not run.

Final review: self-review per the user scope excluding sub-agent work; no critic
was created or run. One cached-overlay invalidation finding was fixed with a
failing-then-passing regression test. GPU preview fitting preserves aspect ratio,
and small-sprite annotation readability has an exact integer-scale regression test.
The implementation remains uncommitted for review; no push or publish occurred.
