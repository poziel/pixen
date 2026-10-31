# Pixen MCP API contract

Contract: `pixen-mcp/v1`. Scope: DEV-233, foundation for the remaining MCP roadmap. This defines the **target API**, informed by the imported artwork workflows and Pixen's document engine. The [implementation reference](../mcp/mcp-reference.md) describes the running API today; DEV-235 implements discovery, project creation/info, layer/frame operations, revision-checked batches and history. Later issues migrate the remaining families; preflight reports actual availability.

**Backward compatibility is not a requirement.** Existing tool names, schemas, defaults, results and errors may be replaced or removed. Build one coherent API; do not retain a legacy namespace, dual dispatcher, shape guessing or deprecated aliases solely to support hypothetical clients. The filename retains the issue's original terminology: compatibility here concerns artwork workflows and document semantics, not old MCP clients. Gallery submission is excluded. Preserve the imported procedures, agents, rules and pixel templates under `playbook/`.

## Authority and implementation boundary

The MCP tool handlers translate arguments into the existing `Session` and Rust document executors (`core/automation/session.rs`, `core/document/edit.rs`, `edit_extra.rs`). It must not edit saved JSON behind the session, maintain a second pixel engine, or round-trip through an export to apply an edit. New editing capabilities belong in those executors first, with executor tests, then in the MCP tool handlers.

The machine-readable [operation inventory](../mcp-compatibility/operations.json) classifies every referenced imported family/operation. Its statuses mean:

| Classification | Meaning |
| --- | --- |
| `adapter` | Existing authority can perform the operation within the stated restrictions; tool/schema refactoring and argument/result translation are still future work. No legacy API must be retained. |
| `new_capability` | Requires new engine, inspection, metadata or tool functionality; unavailable until explicitly advertised. |
| `unsupported` | Deliberately refused by v1; the reason and recovery action are part of the contract. |

Each row names the source guidance `command`/`op` and its canonical `target_tool` / `target_op`. `implementation_tools` are pointers to today's implementation for developers reusing the engine; they are not APIs to preserve. `alternatives` use the target tool names. The inventory's `api_policy` explicitly permits breaking changes and selects direct tools, `preflight` discovery and snake_case arguments.

The inventory includes bare/default calls, batch wrappers and features expressed as arguments, not only `op` strings. Unknown operations and unknown semantic options are unsupported, even if another operation in the family is available. Existing call examples in `playbook/skills/pixen` are migration inputs, not a constraint on the new API. The `skills/pixel-art` bundle contains artistic advice, not another MCP command surface. The source pointers in each inventory row cover imported skills, agents and rules.

| Family | Existing-executor adapter subset | New or refused behavior |
| --- | --- | --- |
| `preflight`, `sprite_info` | Project info | Capability/mode readiness discovery |
| `sprite_manage` | New project, standalone save | Save As, slices; refuse upstream color-mode emulation |
| `layer`, `frame` | Flat layers, frame edits, atomic batches | Groups/locks; refuse cross-document copying |
| `cel` | Independent cell copying | Offsets, tweening, oscillation |
| `draw` | Exact pixels, unit-stroke bounded shapes, clear, blit | Grid codec, text, polylines, gradients, dither, tolerant/color-locked operations |
| `palette`, `recolor` | Exact palette read/set/append and whole-scope index remap | Presets/loaders, ramps, extraction, perceptual or masked recoloring |
| `transform`, `select` | Bounded region flips, right-angle rotation, translation | Scale/outline/masks; refuse lossy transforms |
| `look`, `read_pixels` | PNG, filmstrip, onion, frame diff, exact cel indices | ASCII codec and reference comparison |
| `validate`, `tag`, `reference`, `tileset` | Native review alternatives are available | Imported checks, tag authoring, references and tilemap workflows need capabilities |
| `export`, `resize_canvas` | Standalone native formats; native canvas resize | Scaled exports and packed atlases; desktop file actions belong to the application |

## One API and direct tool discovery

Expose operations as direct MCP tools. Use the workflow families in the inventory as the target tool names: `sprite_manage`, `sprite_info`, `layer`, `frame`, `cel`, `draw`, `palette`, `recolor`, `transform`, `select`, `tag`, `reference`, `look`, `read_pixels`, `validate`, `export`, `tileset` and `resize_canvas`. Add `preflight` for capability/readiness discovery and `edit_batch` for mixed-family atomic edits. `undo`, `redo` and focused review/checkpoint tools can remain when they have a useful distinct role. There is no `execute_guidance` or legacy forwarding API. A family is advertised only once its supported operations have been implemented. The inventory records the disposition of guidance operations, not a requirement to reproduce every upstream API detail.

Use consistent **snake_case** argument names. Convert imported camelCase examples such as `fromFrame`, `toFrame`, `selectionOnly`, `paletteLock` and `durationMs` to `from_frame`, `to_frame`, `selection_only`, `palette_lock` and `duration_ms` in the new tool schemas. Preserve the artistic procedure while updating executable call examples during the relevant implementation tasks. `command` in the inventory is a family identifier, not an extra public dispatch envelope.

Common arguments belong directly in each tool's `arguments`: `project_id`, `expected_revision`, and the advertised frame/layer selectors. Use `op` for a single family operation and `ops`/`batch` for ordered transactions. `draw` uses `ops:[{kind:..., ...}]`; replacing its old single-`operation` schema is intentional. Do not inspect an incoming shape to decide between old and new `draw` semantics. For mixed-family edits, `edit_batch` takes `{project_id, expected_revision, operations:[{tool, arguments}, ...]}`. Nested entries use canonical direct-tool arguments without their own project/revision; the batch's common fields apply to all operations. Nested batches and lifecycle/file actions are rejected.

For example, the target `draw` tool receives these arguments directly:

```json
{
  "project_id": "project-1",
  "expected_revision": 4,
  "layer": "base",
  "frame": 1,
  "ops": [{"kind": "pixels", "pixels": [{"x": 0, "y": 0, "palette_index": 1}]}]
}
```

Resolve the old `draw` name collision by **replacing the old schema** with this canonical one. No parallel `draw_legacy`, native/guidance split or wrapper is needed. Existing client configurations and allowlists may change as part of the migration. Current API examples in the reference are implementation evidence, not promises of continued support.

Clients initialize, inspect `tools/list` and its exact `inputSchema` values, then call the advertised `preflight` tool. It returns `contract:"pixen-mcp/v1"`, `mode` (`standalone` or `desktop`), `frame_base:1`, `coordinate_base:0`, `palette_semantics:"exact_index"`, `revision_required:true`, and per-operation records with `tool`, `op`, `available`, `classification`, `constraints`, `reason`, `alternatives` and `owner` (`session` or `application`). Alternatives name tools and operations from the target API or a desktop UI action, not legacy aliases. Include the current project ID/revision when attached. `ready` means the selected owner/connection is usable; `has_project` separately identifies an active document. Tool schemas and preflight must describe supported argument subsets, not just family names. Host prefixes are presentation details, not API negotiation.

Availability is runtime truth, independently of roadmap classification. On today's binary, absence of `preflight` means the target API is not implemented; do not send the target `draw` example to that binary. Upgrade/reconnect when the replacement is available. No automatic legacy fallback, live-to-headless fallback or new process on connection failure. Versioning identifies the advertised contract; it does not impose an old-client support window during this pre-release refactor.

## Addressing and ownership

* A standalone session owns one active project. Create/open returns its opaque `project_id`; the client carries it on subsequent calls. A project-scoped call may omit it only when the connection has exactly one active document. An explicit ID must match; never silently replace it with the active document.
* Desktop calls use the attached artwork and its session. UI tab/selection changes never retarget a queued call. Resolve the artwork and revision under the same state lock used to commit. If the document changes, fail and refresh.
* `sprite`/`toSprite` selectors, including upstream `#<id>`, are not Pixen project IDs. Cross-document copies/duplicates are unsupported in v1. Never guess a file from its basename or use a path to bypass ownership.
* Layer names are exact, case-sensitive matches to the current document. Resolve once to stable IDs at the guarded revision. Zero matches is `target_not_found`; multiple matches is `ambiguous_target`, returning candidate IDs/names. Never choose the first match or rename the user's layers. Flat names containing `/` remain literal; do not interpret them as group paths. Hierarchical paths require a future advertised grouped-layer capability. New layers/frames get never-used valid engine IDs; return those IDs to clients. Labels are not IDs.
* Target `frame`, `from_frame`, `to_frame`, tag endpoints and `expect` frame ranges are **one-based timeline positions**, with inclusive endpoints. Stable IDs are the internal authority; clients may instead supply `frame_id` or `layer_id` explicitly. A name/position and its corresponding ID are mutually exclusive; reject conflicting selectors. Structural `index` is zero-based. Resolve positions at the guarded revision; `frame:1` means ordered frame zero, even after reorder. Reject zero/negative/out-of-range/fractional frames. A missing frame is allowed only on a single-frame document or an operation explicitly scoped to all frames; otherwise require it. A missing layer is allowed only with exactly one eligible layer or an explicitly advertised all-layer operation. No hidden selection state.
* Imported prose sometimes calls the first frame "frame 0" (particles rule) and some template captions number panels from zero. These are artistic descriptions; executable target API calls use one-based frames. Do not auto-detect bases.

| Action | Standalone | Desktop |
| --- | --- | --- |
| Read/draw/edit/history/review | Session authority | Same attached session and shared UI history |
| Create | Explicit new `.pixen` / `.pixen.json` destination, palette required | App-managed temporary destination; only when creation is legal in the attached session |
| Open, save, save-as, finish, export, preview window | Session/file tools with explicit paths | Application owns these; return `application_owned`, with UI action; no filesystem fallback |
| Inspect PNG / filmstrip / onion | Read-only evidence | Read-only evidence; existing Follow AI policy applies |

Today's desktop allowlist exposes 26 tools and standalone exposes 31. These counts and names are observations, not target requirements. Replace the allowlist and provider schemas along with the API. Preserve **ownership**, not the old list: file/lifecycle actions remain application-owned on desktop even if standalone moves them under `sprite_manage` or `export`. Enforce ownership for indirect calls inside batches as well. `mode:"live"` means desktop attachment; `mode:"headless"` means standalone. Reject a mode conflicting with the connection.

## Geometry, frames and palette

Canvas coordinates are integer pixels, origin top left, x right, y down. Rectangles use `{x,y,width,height}`, with positive dimensions and half-open extents. Line endpoints are inclusive. Every source/destination must lie wholly inside the canvas; reject overflow instead of clipping or wrapping. Source-image regions are source pixels and must be distinguished from target-canvas coordinates. Scale is positive integer nearest-neighbor. Rotation operations accept only multiples of 90 degrees. Imported shape options must be validated; the engine's ellipse rasterization is authoritative, not an unverified claim that it matches every rulebook template. Frame duration is positive finite milliseconds, no seconds/fps inference. Layer order is bottom-to-top; final insertion/reorder positions are zero-based.

Palette indices are zero-based exact document slots, including duplicate RGBA entries. Index 0 is **not necessarily transparent**. Resolve alpha through the palette; native creation appends a transparent slot if necessary. Never silently sort, deduplicate, renumber, replace or widen the palette. An exact RGBA string may resolve only to one unique existing slot; duplicate matches require an index. Normalize `#RRGGBB` to opaque `#RRGGBBAA` before exact lookup. Perceptual snapping, ΔE reports, RGB/indexed color-mode emulation and palette extraction require new capabilities. Native documents use indexed cels with RGBA palette entries; calling them unrestricted upstream RGB mode would be misleading.

For future `grid`, rows are equal-width nonempty strings; legend glyphs map to exact indices or uniquely resolved colors. Reserve `.` for a transparent pixel. Glyphs are single printable ASCII characters; width counts those characters, not bytes of arbitrary Unicode. Reject a legend that redefines `.`. Default transparency writes the document's transparent index and erases the cell; `transparent:"skip"` emits no edit for that cell. Unknown glyphs/ragged rows fail before committing. Readback legends must carry palette indices and revision, so duplicate colors and independently read regions do not lose slot identity. The existing `Drawing::SetPixelRows` executor writes every entry, including transparent slots; it has no skip token. It may be reused internally without retaining a public legacy call shape. The ASCII/grid workflow needs a new codec capability before being advertised. `palette_lock:false`, tolerance, masks, stroke thickness and interpolation are never silently discarded.

## Transactions, history and external changes

All document mutations require `expected_revision` directly in tool arguments. Imported examples omit it; migrated clients must discover the revision and add it. Reject omission rather than filling in the latest revision immediately before editing. Resolve names, positions, palette colors and IDs under that revision and the session lock. A concurrent user edit makes the entire call stale.

`ops`, `batch`, multi-frame durations and expanded operations become **one** `Session::apply` transaction: one candidate, validation, commit and undo step. Validate all options and ownership before doing any work. References to preexisting targets are bound before execution; newly created targets may be addressed by a unique name introduced earlier in the same batch, tracked against that candidate. Reject duplicates/ambiguity there too. Never loop over public mutation tools and call that atomic. Lifecycle/filesystem operations cannot be mixed into edit batches. Reject an expansion exceeding the engine's 1..10000 operation limit; keep complete UTF-8 requests below the recommended 200 KiB budget. Do not silently split batches.

Failure leaves document bytes, revision, undo/redo, used-ID allocation and recovery state unchanged. Successful edits, including no-op edits, advance revision once; undo/redo also advance monotonically, so an old revision never becomes current again. Existing history limits apply. Checkpoints are session-local review evidence, not saved files or undo entries. Save/export are separate explicit actions and retain existing external-file-change protections; tool handlers must never retry with overwrite or replace an externally modified file automatically.

## Results and errors

Use one result format across the refactored tools: JSON metadata in MCP `structuredContent`, described by each tool's `outputSchema`, and actual PNG MCP content for visual operations. Text content can summarize the result; clients read metadata directly rather than parsing serialized JSON out of text. Results contain `contract`, `tool`, `op`, `project_id`, `revision`, `resolved_targets` and operation-specific `result` (omit inapplicable fields on discovery/lifecycle calls). The top-level `revision` identifies the resulting document; the `result` object contains `changed_bounds`, `changes`, `undo_steps` and `redo_steps` for mutations, or tool-specific data such as `pixels` for reads. These values come from the executor. Structural bounds may conservatively cover the canvas. Reads identify the inspected revision. Existing response layouts may be replaced; do not maintain a separate legacy parser path. Never imply a PNG was visually reviewed just because it was returned or a window was opened.

Tool failures set `isError:true` and structured JSON containing `contract`, `error:{code,message,tool,op,capability,mode,retryable,alternatives}` plus current `project_id`/`revision` when known. Candidate lists and invalid field paths belong in `error.details`. Required codes and client recovery:

| Code | Recovery |
| --- | --- |
| `unsupported_tool`, `unsupported_operation`, `unsupported_option` | Read capabilities; choose the stated available alternative or report the missing feature. |
| `capability_unavailable` | A classified future feature is absent; upgrade/reconnect only if a build actually implements it. |
| `application_owned` | Use the named desktop UI action; do not open a standalone session behind the user's back. |
| `mode_mismatch`, `not_ready`, `no_project`, `project_mismatch` | Attach/select the intended owner and discover again. |
| `target_not_found`, `ambiguous_target` | Inspect IDs/names; provide an explicit unambiguous target. |
| `revision_required`, `stale_revision` | Refresh state and reconsider the edit; never replay automatically. |
| `invalid_arguments`, `out_of_bounds`, `palette_mismatch`, `locked_layer`, `resource_limit` | Correct the field/scope/budget; original batch did not apply. |
| `external_change`, `io_error` | Preserve current work, inspect the file conflict or destination; no force-save fallback. |

Errors must name the specific missing capability, not claim the whole family works. Example: `cel.oscillate` reports missing `cel_interpolation` and suggests explicit keyframes using `frame` and `draw`. A completed validation with findings is a successful result with `passed:false`, not a transport/tool error. Never map all imported `validate` checks to existing review rules and claim full coverage: outline, banding, AA, layer overlap/ranges and unsaved/tag policies need their own advertised checks. No aesthetic recommendations become engine invariants.

For example, an unavailable imported operation returns an actionable tool error:

```json
{
  "contract": "pixen-mcp/v1",
  "error": {
    "code": "capability_unavailable",
    "message": "Cel oscillation is unavailable; create explicit keyframes instead.",
    "tool": "cel",
    "op": "oscillate",
    "capability": "cel_interpolation",
    "mode": "desktop",
    "retryable": false,
    "alternatives": ["frame", "draw"]
  }
}
```

This is the target error payload. Replace the current plain-text tool errors during implementation; there is no requirement to maintain both formats. Unknown tool names may still be rejected at the MCP protocol routing layer.

## Executable scenarios and adoption

Run `cargo test --locked --test mcp_protocol guidance_contract` for the reusable [scenario fixtures](../mcp-compatibility/scenarios.json) and native wire tests in `tests/automation/mcp_compatibility.rs`. Run `cargo test --locked --lib bridge_edits_the_desktop_document_and_filters_file_actions` for current desktop ownership enforcement.

Fixtures preserve representative original guidance inputs, direct target tool calls, current-executor calls and expected observable state. DEV-235 scenarios marked `execute_target` execute the revised target calls directly and assert structured metadata. Remaining drawing/inspection/delivery scenarios still execute recorded current calls as engine evidence until their roadmap issues migrate those families. Current calls are temporary executable evidence of the engine semantics. They must be replaced or moved to executor-level tests as tool handlers are refactored, rather than becoming a reason to retain old names or schemas. `expected` / `error_contains` assert current execution; `target_expected` describes the canonical structured result, and `target_error` identifies the target error code. The runner reads structured metadata for revised tools; unchanged native tools still provide text metadata during the staged migration.

Tests must not freeze the old tool counts, schemas, absence of new families, or rejection of the new `draw` shape. Instead, verify the target direct-call fixture structure, revision requirements, operation classifications and document-safety outcomes. Future implementation issues execute each `target` with its `name` and `arguments` directly via `tools/call` and compare the same state, revision and history assertions. Add grouped paths, selections, palette quantization, errors and ownership cases as those features become available. Never mark pending features available merely to make a scenario pass.

The test-only addressing oracle checks ambiguous/missing names, literal slash names and one-based positions against real project metadata. It remains a reference policy; runtime wire tests separately exercise ambiguous/missing names, literal slashes and guarded positions.

DEV-234 can package this contract/discovery instructions and update client-facing call references together with the implementation. DEV-235 and later issues refactor or replace tool families directly, sharing these fixtures and the Rust executors. They may remove superseded tools and update clients/allowlists in the same change. Preserve the artwork procedures and templates, and the correctness invariants defined here; do not preserve obsolete API mechanics for their own sake.
