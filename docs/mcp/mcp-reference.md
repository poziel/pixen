# MCP server and tool reference

For the full roadmap contract, see the [target MCP API contract](../specs/mcp-compatibility.md). Current tool names, schemas and response formats may be replaced without legacy aliases. This reference describes the implementation today; it is not a backward compatibility guarantee.

This reference documents the current Pixen source implementation. Tool availability and per-operation readiness are discovered through `tools/list` and `preflight`. Project, structure and batch commands implement `pixen-mcp/v1`; drawing, inspection and delivery families migrate in later roadmap issues. These are MCP tool names, called through `tools/call`, rather than separate HTTP URLs. See [the MCP guide](README.md) for longer authoring workflows.

## How the server works

The client launches `pixen-mcp` and communicates over standard input/output using JSON-RPC through the Rust `rmcp` SDK. Stdout carries protocol messages; stderr carries diagnostics. The server advertises tools, with JSON Schemas generated from its Rust argument types. It also advertises MCP resources and workflow prompts backed by the embedded playbook; see [guidance discovery](guidance.md).

The normal protocol sequence is:

1. `initialize`: negotiate the protocol and read server information/capabilities.
2. `notifications/initialized`: complete client initialization.
3. `tools/list`: discover tool names, descriptions and `inputSchema` values.
4. `tools/call`: invoke a named tool with an `arguments` object.

For example, after initialization:

```json
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
```

```json
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"sprite_info","arguments":{"project_id":"project-1"}}}
```

There is no public HTTP listener or REST route for each tool. The desktop uses an internal authenticated TCP bridge on `127.0.0.1` with a dynamically assigned port. Its proxy authenticates with a random token before forwarding MCP traffic. This is an app-managed connection, not an HTTP API to configure manually.

### Standalone and desktop modes

| Mode | Server name | Document ownership | Available tools |
| --- | --- | --- | --- |
| `pixen-mcp` | `pixen` | Server owns one active editing project | All advertised tools |
| `pixen-mcp --viewer` | `pixen` | Same session; automatically opens a read-only desktop preview on create/open | All advertised tools |
| Desktop document bridge | `pixen-desktop` | Desktop owns the active artwork, saving and project lifetime | Document tools |

The standalone-only file tools are `open_project`, `save_project`, `export_project`, `open_preview` and `finish_project`. Desktop excludes them from discovery and returns structured `application_owned` errors for direct calls. Desktop `sprite_manage op:"new"` omits `output_path`; Pixen supplies its temporary destination. Supplying a path on desktop is rejected. Creation is legal only without existing artwork. A bridge remains bound to that document and stops with its owning application; tab changes never retarget queued requests.

### Setup

```powershell
cargo build --locked --release --bin pixen-mcp
.\target\release\pixen-mcp.exe
```

Example Codex configuration:

```toml
[mcp_servers.pixen]
command = 'D:\Dev\1-codes\pixen\target\release\pixen-mcp.exe'
args = []
```

Add `--viewer` to `args` for automatic previews. Preview windows require the sibling Pixen desktop executable and an interactive desktop. Headless edits and PNG inspection do not require a window. Restart the MCP connection after rebuilding; `tools/list` describes the binary actually running.

## Shared argument and result conventions

`read_guidance` is a read-only, document-independent tool available in both modes. Its required `uri: string` names an exact bundled resource, such as `skill://index`, `skill://studio` or `rules://00-core-principles`. It returns text content and sets `isError` for unknown URIs. It does not accept local paths or change artwork/history. The same text is available through `resources/read`; see [guidance discovery](guidance.md) for URI mapping and workflow prompts.

In the tables below, fields are **required unless marked optional or given a default**. The revised project/structure/batch/history tools allow omitted `project_id` for the connection's single active document; an explicit ID must match. Other tool schemas still require it. `sprite_manage op:"new"` and `open_project` do not address an existing project. Use returned IDs rather than assuming their values.

| Value | Meaning |
| --- | --- |
| `expected_revision` | Nonnegative integer matching the current project revision |
| `frame_id`, `layer_id` | Stable string IDs, not timeline/list indices |
| `palette_index` | Zero-based integer into the exact `#RRGGBBAA` palette |
| `region`, `crop` | Object `{x, y, width, height}`; integer coordinates, positive integer dimensions, wholly inside canvas |
| `scale` | Positive integer nearest-neighbor enlargement; defaults vary by tool |
| `Scope` | Object with optional `frame_ids: string[]`, `layer_ids: string[]`, `crop`; omitted selections use all matching frames/layers/full canvas |

Coordinates start at the top left, with x increasing right and y increasing down. There is no silent clipping. Opacity is a finite number from 0 through 1; durations are positive finite milliseconds. New frame/layer IDs must satisfy Pixen identifier validation (1–128 ASCII letters, digits, underscores or hyphens) and must never have been used in that project, including IDs removed or undone. Renaming/reordering does not change stable IDs.

The revised tools (`preflight`, `sprite_info`, `sprite_manage`, `layer`, `frame`, `edit_batch`, `resize_canvas`, `undo`, `redo`) return metadata in MCP `structuredContent`, described by `outputSchema`. Read `{contract,tool,op,project_id,revision,resolved_targets,result}` directly. The `result` object contains operation data, including mutation bounds/change records and history counts. Errors set `isError:true` and carry `error:{code,message,tool,op,capability,mode,retryable,alternatives,details}` plus known project/revision. Missing revisions, stale edits, ambiguous/missing targets, unavailable capabilities and unsupported options fail explicitly. SDK-invalid parameter types may produce protocol errors.

Remaining drawing, inspection, review and standalone file tools retain their currently advertised schemas and text metadata until their roadmap migrations. Image tools return actual PNG MCP content. Do not send future drawing/grid schemas to the current `draw` tool; preflight marks those target operations unavailable.

Edits resolve under the session lock, execute against a candidate and commit once. A successful transaction, including a no-op, creates one undo step and advances revision once. Failure leaves document, recovery, used-ID allocation and history unchanged. Undo/redo advance monotonically. Original name and frame-position selectors bind to the guarded revision; a unique layer name created earlier in the same batch may address that new layer. Nested commands may not override project/revision.

Drawing mutations usually return `revision`, `changed_bounds`, `changes`, `undo_steps` and `redo_steps`. Drawing bounds describe actual changed indices, including linked targets; structural operations use conservative canvas bounds. History operations return revision, canvas bounds and history counts. No mutation automatically generates an inspection PNG.

Standalone transactions persist recovery before acknowledgement. The desktop can defer recovery to its background worker; it also updates its temporary container after bridge mutations. If updating a desktop temporary container or turn checkpoint fails after commit, the tool remains successful and reports `result.persistence_warning`; the returned revision identifies the applied edit. Preserve the work and use application Save/Save As rather than replaying that edit.

## Project lifecycle tools

### `preflight`

Optional `mode:"headless"` for standalone or `mode:"live"` for desktop must match the connection. Returns readiness separately from `has_project`, mode, Pixen/contract versions, coordinate base 0, frame base 1, exact-index palette semantics, revision requirement, viewer configuration and operation records. Each record reports actual availability, classification, constraints, ownership, reason and alternatives. Absent roadmap capabilities remain unavailable even when a current native tool provides a narrower alternative.

### `sprite_manage`

`op:"new"` requires positive `width`, `height` and an explicit `palette` of `#RRGGBBAA` entries. Standalone also requires a new `output_path` ending in `.pixen` or `.pixen.json`, with an existing parent; desktop owns the destination and rejects that field. A transparent slot is appended if needed. Creates one transparent frame/layer and returns stable IDs at revision 0. There must be no active project. No RGB/indexed-mode emulation is exposed. File operations in this family remain unavailable pending the delivery issue; desktop reports `application_owned` for them. The existing standalone file tools below remain available meanwhile.

### `open_project` — standalone only

Arguments: `path: string`, optional `output_path: string`. Load a supported input as an editable project: Pixen containers/authoring JSON, legacy Pixloom/Spritex, Pixelorama `.pxo`, supported Aseprite and raster image/animation inputs through the shared loader. Native import restrictions still apply.

An existing `.pixen` defaults to saving back to itself; other inputs default to a sibling `.pixen` file. Explicit output must be `.pixen` or `.pixen.json`. Unrelated existing destinations are rejected. Returns full project info at revision 0, assigns missing stable frame IDs, and starts fresh session history.

### `save_project` — standalone only

Arguments: `project_id`. Validate and atomically save to the session destination without ending it or changing revision/history. Returns `saved_path` and `revision`. Refuses unrelated existing files and changes made on disk since open/last save. Failure preserves the session.

### `export_project` — standalone only

| Argument | Type / behavior |
| --- | --- |
| `project_id` | String |
| `output_path` | String; extension selects output format |
| `overwrite` | Boolean, default `false` |
| `frame_id` | Optional string; selects a single frame for image output |
| `loop_animation` | Boolean, default `false`; request looping animated image output |

Exports `.pixen`, `.pixen.json`, `.ase`/`.aseprite`, `.pxo`, or supported images (PNG, GIF, APNG, WebP, JPEG, BMP, TIFF, SVG). Video export uses the CLI. Project/native exports preserve the full timeline and reject `frame_id` or `loop_animation`. Animated image output uses the full timeline unless a frame is selected; static output defaults to the first frame. Looping requires a compatible animation and conflicts with selecting one frame. Format-specific timing/alpha restrictions can reject export.

Returns `exported_path` and `revision`. Does not change the project destination, revision or history, and refuses to replace the active project or recovery file even with `overwrite: true`.

### `finish_project` — standalone only

Arguments: `project_id`. Save using the same protections as `save_project`, then release the session, remove recovery and close its managed viewer. Returns `saved_path` and `revision`. Save failure keeps the project/viewer available. History and review checkpoints end with the released session.

## Editing tools

### `edit_batch`

Arguments: optional `project_id`, required `expected_revision`, and `operations:[{tool,arguments}]`. Nested arguments are the direct tool arguments without project/revision. Supports single `layer`/`frame` edits and current `draw`, `edit_palette`, `copy_region`, `transform_region` arguments. Lifecycle/file actions, resize, list operations and nested batches are rejected. Expand to 1–10,000 engine operations and commit once; never split implicitly. Keep requests below the recommended 200 KiB UTF-8 budget.

```json
{"expected_revision":0,"operations":[
  {"tool":"layer","arguments":{"op":"create","name":"ink"}},
  {"tool":"frame","arguments":{"op":"add","count":2,"duration_ms":100}},
  {"tool":"layer","arguments":{"op":"set","layer":"ink","opacity":0.8}}
]}
```

### `draw`

Arguments: `project_id`, `expected_revision`, `frame_id`, `layer_id`, `operation: Drawing`. Runs one drawing operation through the batch executor. The nested `operation` has `type` and the following fields:

| Drawing `type` | Fields | Effect |
| --- | --- | --- |
| `set_pixels` | `pixels: [{x,y,palette_index}, ...]` | Sparse writes; later repeated coordinates win |
| `set_pixel_rows` | `x`, `y`, `rows: integer[][]` | Dense rectangular patch; nonempty equal-width rows, including transparent overwrites |
| `line` | `x1`, `y1`, `x2`, `y2`, `palette_index` | Bresenham line including both endpoints |
| `rectangle` | `x`, `y`, `width`, `height`, `palette_index`, `filled: boolean` | Filled rectangle or border |
| `ellipse` | `x`, `y`, `width`, `height`, `palette_index`, `filled: boolean` | Pixel-center ellipse; outline uses four-neighbor boundary |
| `fill` | `x`, `y`, `palette_index` | Four-connected flood fill matching exact source indices |
| `erase_region` | `x`, `y`, `width`, `height` | Replace rectangle with transparency |
| `clear_cel` | None | Replace whole target cel with transparency |
| `clear` | `palette_index` | Legacy whole-cel fill with an explicit index |

Drawing rejects locked layers and out-of-range colors/bounds. Explicit cel link groups share edits; unlinked reused cel storage detaches before writing. Transparency operations append a transparent palette entry only if necessary.

### `layer`

Common arguments: optional `project_id`, `expected_revision` required for edits, and `op`. Targets use exact, case-sensitive `layer` names or explicit `layer_id`, mutually exclusively. Omit a target only with one eligible layer. Ambiguous names return candidate IDs/names. Slash characters are literal names; groups and cross-project duplication are unavailable.

| `op` | Fields | Behavior |
| --- | --- | --- |
| `list` | No mutation fields | Read ordered layers, stable IDs and current properties |
| `create` | `name`, optional `index`, `layer_id` | Add a transparent flat layer on every frame |
| `duplicate` | Target, `name`, optional `index`, `new_layer_id` | Independent cel copies |
| `delete` | Target | Remove, retaining at least one layer |
| `rename` | Target, `name` | Change name while preserving ID |
| `reorder` | Target, `index` | Move to zero-based final bottom-to-top position |
| `set` | Target, `visible` and/or `opacity` | Update visibility or opacity 0..1 |
| `batch` | `ops:[{op,...}]` | One atomic transaction of single-layer commands |

Creation allocates never-used IDs unless an explicit valid never-used ID is supplied. Existing native `paste_cells`, `clear_cells`, `copy_cells`, `link_cells`, `unlink_cells` also use direct `op` and their exact discovered fields (stable layer/frame IDs, never implicit selection). They remain independent cell/link capabilities, not group support. The old `operation:{type:...}` wrapper is removed.

### `frame`

Targets use a one-based `frame` position or `frame_id`, mutually exclusively. Missing targets are legal only on single-frame documents. Structural `index` remains zero-based. Positions bind to the starting revision, even after earlier reorder operations in the batch. Newly created frames are returned as IDs and may be used by ID later in that batch.

| `op` | Fields | Behavior |
| --- | --- | --- |
| `list` | No mutation fields | Read ordered IDs, positions and timing |
| `add` | Optional `count` (default 1), `duration_ms` (default 100), `index`, `frame_id` | Add transparent frames; explicit ID requires count 1 |
| `duplicate` | Target, optional `count`, `index`, `new_frame_id` | Independent copies; explicit new ID requires count 1 |
| `set_duration` | Target and `duration_ms`; or `frames:[positions]` with `duration_ms`/`durations`; or timeline-wide `durations:[ms,...]` | Positive finite milliseconds; array length must match targeted frames; one undo step |
| `delete` | Target | Remove, retaining at least one frame |
| `rename` | Target, `name` | Change display name |
| `reorder` | Target, `index` | Move to zero-based final position |
| `batch` | `ops:[{op,...}]` | One atomic transaction |

Insertions accept 0..count, reorders 0..count-1. IDs are allocated unless supplied explicitly. Duplicate copies are holds, not automatically generated motion. Existing tag constraints remain enforced.

### `edit_palette`

Arguments: `project_id`, `expected_revision`, `operation: PaletteOperation`.

| Operation `type` | Fields beyond `type` | Effect |
| --- | --- | --- |
| `append_colors` | Nonempty `colors: string[]` | Append exact `#RRGGBBAA` entries |
| `set_color` | `palette_index`, `color: string` | Change one entry globally, retaining a transparent entry for empty cels |
| `remap_color` | `from_index`, `to_index`, optional `frame_id`, optional `layer_id` | Replace exact indices globally or in selected scope; respects locks and intentional links |

Indices are not renumbered or deduplicated. Returns the standard edit result.

### `copy_region`

Arguments: `project_id`, `expected_revision`, `source_frame_id`, `source_layer_id`, `region`, destination `frame_id`, `layer_id`, `x`, `y`, and optional `transform` (default `none`). These fields are top-level tool arguments. Snapshot-copy a rectangle, including transparency and exact palette indices. Overlaps are safe; source and transformed destination must both fit the canvas.

### `transform_region`

Arguments: `project_id`, `expected_revision`, `frame_id`, `layer_id`, `region`, destination `x`, `y`, and required `transform`. Snapshot the region, clear its old rectangle, then paste at the destination in one transaction.

Both region tools use transforms `none`, `flip_horizontal`, `flip_vertical`, `rotate90`, `rotate180`, `rotate270`. Rotations are clockwise; quarter-turns swap output dimensions. `none` performs a plain copy/move. Lock/link behavior matches drawing; both return the standard edit result.

### `resize_canvas`

Arguments: `project_id`, `expected_revision`, positive integer `width`, `height`. Resize every cel, anchoring pixels at the top left: smaller dimensions crop; larger dimensions pad transparently. Does not scale artwork. Preserves palette indices, stable IDs and timing. Returns updated project info. A changed size is one undo step; unchanged dimensions still advance revision and create one undo step. Invalid sizes/budgets or a stale revision leave artwork/history unchanged. This is a separate tool, not an `edit_batch` operation variant.

### `undo`

Arguments: `project_id`, `expected_revision`. Undo one committed transaction. One batch is one step; revision increases rather than returning to its old value. Returns updated revision, canvas bounds and history counts. Empty history is an error. History is session-local, bounded to 32 snapshots/64 MiB total.

### `redo`

Arguments: `project_id`, `expected_revision`. Reapply one undone transaction, increasing revision. Returns the same history result as undo. Empty redo is an error; a new successful edit clears redo.

## Inspection and preview tools

### `sprite_info`

Optional `project_id`. Returns current revision, dimensions, palette, ordered layers/frames with stable IDs and one-based frame positions, cel/link metadata, and history counts/limits. `color_model:"palette_index_with_rgba_entries"`, `palette_semantics:"exact_index"` and `upstream_color_modes:false` distinguish Pixen's model from upstream RGB/indexed modes. `index_zero_is_transparent` is computed from this project's palette alpha, never assumed. Duplicate RGBA slots retain their distinct indices.

### `get_pixels`

Arguments: `project_id`, `frame_id`, `layer_id`, `region`. Return at most 4096 row-major pixels with `x`, `y`, `palette_index`, `rgba`, plus revision/target/source metadata. Reads exact uncomposited cel indices/colors; does not apply visibility, layer opacity or cel opacity. Rejects out-of-bounds regions instead of clipping.

### `get_preview`

Arguments: `project_id`, optional `scale` (default 8). Return the **first ordered frame's** composite as PNG plus `revision`, original `width` and `height`. Does not follow viewer selection and does not return an animation.

### `inspect_project`

Arguments: `project_id`, optional `frame_id` (literal default `frame-1`), optional `scale` (default 8), optional `crop`. Return composite PNG plus `project_id`, `revision`, `frame_id`, original dimensions, `source` rectangle and scale. Imported projects may have no `frame-1`; pass a discovered ID explicitly. Synchronizes the managed inspector's frame/composite mode while Follow AI is on.

### `get_contact_sheet`

Arguments: `project_id`, optional ordered unique `frame_ids: string[]` (default whole timeline), `scale` (default 8), `columns` (default 4, range 1–64). Render 1–64 frames with stable-ID/duration labels. Returns PNG plus canvas/sheet dimensions, frame image bounds, revision and `distinct_composites`. Identical rendered holds count once. Sheet output is limited to 16 million pixels; reduce scale or select fewer frames if necessary. Does not verify temporal playback.

### `get_onion_skin`

Arguments: `project_id`, `frame_id`, optional `reference_frame_id`, optional `layer_id`, `scale` (default 8). Return a PNG showing the current pose over faint red previous and cyan next poses, without wrapping at timeline ends. An explicit reference replaces neighbors with that red reference. Optional layer isolates the analysis. Returns revision, target/reference IDs, mode, dimensions, scale and analysis metadata; synchronizes an open Follow AI inspector.

### `compare_frames`

Same arguments as `get_onion_skin`. Reference defaults to the previous frame, or self for the first frame. Return exact composited-RGBA difference evidence: changed pixels are pink (including disappearances), unchanged pixels faint. PNG metadata includes `changed_pixels`, revision and target/reference/mode. Optional layer isolates comparison; synchronizes an open Follow AI inspector.

### `inspect_layer`

Arguments: `project_id`, `frame_id`, required `layer_id`, optional `scale` (default 8). The shared schema also accepts optional `reference_frame_id`, but this tool renders an isolated layer rather than a comparison. Return isolated PNG even for hidden layers, with revision/mode/target metadata. Does not change visibility; clears onion/difference mode in an open Follow AI inspector.

### `open_preview` — standalone only

Arguments: exactly one of `project_id: string` or `path: string`. Show, restore and request activation of a read-only desktop preview for the active project or an existing supported file. Canonical paths identify reusable windows. Existing files are never edited; closing a window does not finish the project.

Returns `opened`, `reused`, `pid`, `activation_requested`, observed `focused`, `attention_requested`, dimensions/frame count, and project revision or file `path` with `revision: null`. Opening uses a bounded acknowledgement wait; failure sets `isError` and returns JSON `{opened:false,error:...}`. OS policy controls focus. This tool shows a window to the user; it provides no PNG evidence to the caller. Use inspection tools for image feedback.

### `report_activity`

Arguments: `project_id`, `message: string`, `busy: boolean`, optional `frame_id`, optional `layer_id`. Publish a trimmed single-line 1–240 character activity. `busy: true` keeps the message/spinner through edits; `false` completes it and restores automatic activity. Optional targets can guide Follow AI. Returns `reported: true` and current `revision`; changes no artwork/history.

## Review, checkpoint and validation tools

These tools produce measurements, not aesthetic scores or automatic repairs. Reviews are read-only for artwork/history; some synchronize a Follow AI review panel. Review reports are stamped with `project_id`, `inspected_revision`, `report_version` and source identity. Respect truncation flags on bounded lists.

### `get_review_sheet`

Arguments: `project_id`, `frame_id`; optional `layer_id`, `crop`, `display_scale` (default 1), `enlarged_scale` (default 8), `views` (default `["color","silhouette","grayscale"]`), `backgrounds` (default `["#FFFFFFFF","#202020FF"]`).

Returns labeled PNG review panels at native/requested scales and source-space panel metadata. Select 1–3 unique views and 1–4 opaque `#RRGGBBAA` backgrounds; scales must be positive. Supports composite or isolated-layer review.

### `manage_checkpoint`

Arguments: `project_id` plus a top-level `action` discriminator:

| `action` | Additional fields | Effect |
| --- | --- | --- |
| `create` | `expected_revision`, `label: string` (1–80 characters) | Capture immutable document baseline; return checkpoint identity/metadata |
| `list` | None | Return checkpoint metadata and storage usage |
| `delete` | `expected_revision`, `checkpoint_id` | Remove a baseline |

Checkpoints are independent of undo/redo and survive save/undo. Create/delete check the revision but do not advance it or change artwork history. Maximum 16 snapshots/64 MiB; exceeding the budget errors without silent eviction. Checkpoints end with the session and are not persisted as document content.

### `compare_checkpoint`

Arguments: `project_id`, `checkpoint_id`; optional `frame_ids`, `layer_ids`, `crop`, `include_images` (default `true`), `scale` (default 8). Compare current artwork to the immutable baseline, returning structural changes, exact index/resolved-color differences and disjoint added/removed/repainted/alpha-only pixel counts/bounds/examples. With images enabled, includes before/current/diff PNG evidence. Report includes baseline and inspected revisions. Does not restore the checkpoint or mutate artwork.

### `analyze_sprite`

Arguments: `project_id`, `frame_id`; optional `layer_id`, `crop`, `alpha_threshold` (default 1, range 1–255), `connectivity` (default 8, either 4 or 8), `include_annotation` (default `true`). Measure occupied bounds, padding, occupancy, centroid, exact palette-index use, rendered colors and connected components. Return JSON measurements and, by default, an annotated PNG. Single-pixel components are observations, not automatically defects.

### `validate_artwork`

Arguments: `project_id`, `rules: ArtRule[]` (1–64 rules). Each rule has a `type`:

| Rule `type` | Fields | Checks |
| --- | --- | --- |
| `canvas_size` | `width`, `height` | Exact canvas dimensions |
| `frame_count` | `min`, `max` | Timeline length within inclusive bounds |
| `frame_duration` | `min_ms`, `max_ms`, optional `frame_ids` | Selected/all frame timings within bounds |
| `allowed_palette_indices` | `indices: integer[]`, optional `scope: Scope` | Exact index usage allowed by caller |
| `max_rendered_colors` | `max_colors`, optional `scope: Scope` | Rendered color-count ceiling |
| `binary_alpha` | Optional `scope: Scope` | Rendered alpha must be fully transparent or opaque |
| `protected_region` | `checkpoint_id`, `region`, `frame_ids: string[]`, optional `layer_id` | Region unchanged against an immutable checkpoint |

Returns rule evidence and aggregate `passed`. A valid rule violation is a successful report with `passed: false`; malformed/unsupported rule arguments are tool errors. Does not impose implicit style rules or modify artwork.

### `audit_animation`

Arguments: `project_id`; optional ordered `frame_ids`, `layer_id`, `crop`, `alpha_threshold` (default 1, range 1–255), `include_loop_transition` (default `false`), `max_centroid_displacement: number`, `max_area_ratio: number`, `scale` (default 1). Displacement thresholds must be finite and nonnegative; area-ratio thresholds must be finite and at least 1. Return PNG contact-sheet evidence and JSON durations, holds/distinct poses, bounds/centroids and consecutive-frame change metrics. The optional loop transition adds last-to-first evidence; thresholds produce caller-defined warnings. Does not require identical loop endpoints or judge artistic quality.

## Complete standalone example

Each object below is the `arguments` value for the named `tools/call`. Substitute the actual IDs/revisions returned by the server. The destination parent must already exist and the output filenames must be new.

```text
sprite_manage
{"op":"new","width":16,"height":16,"palette":["#00000000","#2048A0FF"],"output_path":"D:/Dev/1-codes/pixen/out/sprite.pixen"}

draw
{"project_id":"project-1","expected_revision":0,"frame_id":"frame-1","layer_id":"layer-1","operation":{"type":"rectangle","x":3,"y":4,"width":10,"height":8,"palette_index":1,"filled":true}}

inspect_project
{"project_id":"project-1","frame_id":"frame-1","scale":8}

save_project
{"project_id":"project-1"}

export_project
{"project_id":"project-1","output_path":"D:/Dev/1-codes/pixen/out/sprite.png"}

finish_project
{"project_id":"project-1"}
```

For desktop-bound AI, use the shared editing/inspection tools; the app handles saving, exporting and closing the artwork.

## Limits and troubleshooting

- Keep complete JSON calls below 200 KiB for client compatibility. This is guidance for AI clients, not a 200 KiB server rejection threshold. Prefer shapes and dense `set_pixel_rows`; split large work into revision-checked batches. Earlier successful batches remain if a later batch fails.
- On stale revision, fetch `sprite_info`, reconcile the current artwork, and use its revision. Do not blindly replay a possibly committed edit.
- `get_pixels` is limited to 4096 pixels. Contact sheets accept 1–64 frames. PNG output and review work have bounded budgets; reduce scale, crop, subset or image inclusion on a limit error.
- History documents must serialize within 64 MiB. History and checkpoints are in-memory session state; recovery contains artwork, not the full protocol session/history/checkpoint store.
- Old running binaries may expose fewer tools or stale descriptions, including `.pixloom` destination guidance. Current source uses `.pixen` / `.pixen.json`. Reconnect after rebuilding and inspect the running binary's schema.

## Implementation map

| Source | Responsibility |
| --- | --- |
| [`core/bin/pixen-mcp.rs`](../../core/bin/pixen-mcp.rs) | CLI flags, storage setup, standalone/proxy entry points |
| [`core/automation/mcp.rs`](../../core/automation/mcp.rs) | Tool registrations, argument wrappers, responses, viewer/activity synchronization |
| [`core/automation/mcp_workflows.rs`](../../core/automation/mcp_workflows.rs) | Workflow translation, discovery, addressing, schemas and structured results |
| [`core/automation/mcp_desktop.rs`](../../core/automation/mcp_desktop.rs) | Document tool allowlist, document-bound interception, authenticated local bridge |
| [`core/automation/session.rs`](../../core/automation/session.rs) | Project lifecycle, revisions, transactions, history, save/export, inspection |
| [`core/document/edit.rs`](../../core/document/edit.rs) | Drawing/layer/frame operation schemas and executor |
| [`core/document/edit_extra.rs`](../../core/document/edit_extra.rs) | Palette and region operation schemas/executor |
| [`core/review`](../../core/review) | Review-sheet, checkpoint, comparison, measurement, rule and animation logic |
| [`core/rendering/preview.rs`](../../core/rendering/preview.rs) | Shared preview loading/rendering and contact sheets |

When adding a tool, update this reference and the desktop allowlist if it should be available to document-bound AI. When adding an operation, update its table as well as checking its generated schema through `tools/list`.
