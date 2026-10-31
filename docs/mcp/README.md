# MCP setup and workflows

Pixen's MCP server lets an AI client inspect and edit pixel artwork through the same Rust document engine used by the desktop. See the [tool reference](mcp-reference.md) for the currently implemented arguments, results, limits, and standalone/desktop availability.

The [target API specification](../specs/mcp-compatibility.md) describes a future refactor. Its examples are not calls supported by today's server.

The [bundled playbook](guidance.md) is available now through MCP resources, workflow prompts and the read-only `read_guidance` tool. Start with `skill://index` and `rules://index`; importing a workflow does not implement every editing capability it describes.

## Setup

Build the headless executables from the repository root:

```sh
bun run build:tools
```

Configure your MCP client to launch `pixen-mcp` by absolute path. For example, replace the path in this Codex configuration:

```toml
[mcp_servers.pixen]
command = 'D:\ArtTools\Pixen\pixen-mcp.exe'
args = ["--viewer"]
```

`--viewer` automatically opens a managed desktop preview. Build the desktop separately with `bun run build`, and keep `pixen`, `pixen-cli`, and `pixen-mcp` together. The current desktop uses Tauri and its native prerequisites; see [dependency setup](../development/dependencies.md). Headless editing and returned PNG images do not need an open desktop. Omit `--viewer` for headless use.

The client starts the server; do not launch a second copy for the same connection. Stdout is reserved for the stdio protocol, and diagnostics go to stderr. Reconnect after updating the executable, then inspect `tools/list` for its actual schema.

## Project lifecycle

1. Call `create_project` with canvas size, palette, and a new absolute output path ending in `.pixen` or `.pixen.json`; or call `open_project` for existing artwork. The parent folder must exist.
2. Retain the returned project, layer, and frame IDs and current revision.
3. Use `draw`, `apply_edits`, or structural editing tools with `expected_revision`. Each successful mutation reports the current revision for the next request.
4. Inspect PNG results with `inspect_project` or `get_contact_sheet`.
5. Call `save_project` to persist and continue, or `finish_project` to save and end the session. `export_project` writes a separate supported output without changing the project's save destination. Video uses the CLI.

Stale revisions, invalid bounds, and unsupported operations fail explicitly. Undo/redo and checkpoints are session-local. Standalone sessions write recovery snapshots before acknowledging edits; save important artwork to a permanent project destination. See the [reference](mcp-reference.md#limits-and-troubleshooting) for recovery and output protections.

## Desktop AI and managed previews

The desktop's embedded artist uses a document-bound bridge. File lifecycle tools are filtered because the application owns opening, saving, and exporting the active document. See [AI art chat](../ai/README.md).

A standalone server can open a managed preview with `open_preview`. The preview follows activity, inspection, and review requests through the shared desktop service. Closing it does not end the MCP session. Ordinary updates do not request window focus; explicit preview activation can. `open_preview` is a user-facing window action, not image feedback to the model.

## Review and validation

[Art-review APIs](art-review.md) provide checkpoint comparisons, measurements, review sheets, animation audits, and caller-supplied validation rules. They return evidence, not an automatic aesthetic verdict. Reports describe a particular revision; inspect again after edits. The proposed independent critic workflow is listed under [specifications](../README.md#proposed-behavior).

## Authoring and delivering animation

An animation request means multiple **visually distinct frames with intentional motion**, not just a static sprite saved in an animation-capable project. Frame creation/duplication does not invent motion or tween pixels. Duplicate frames are useful holds or starting points; draw the changed poses explicitly.

Recommended sequence: **block out → review → create keyframes → add useful intermediates → inspect contact sheet and playback → correct → save/export → open user preview**. Keep one revision-checked `apply_edits` batch per logical transaction. `edit_frame` uses that same executor. `add_frame` appends by default; `duplicate_frame` inserts immediately after its source by default. Specify zero-based `index` explicitly when assembling an ordered timeline. Duplication copies all layers into independent cels; subsequent drawing cannot alter its source. Reordering preserves stable IDs. Durations are positive finite ms.

Compact MCP sequence (substitute returned project/revision IDs):

```text
get_project_info(project_id)                         # discover frames/layers
inspect_project(project_id, frame_id="frame-1")      # review blockout PNG
edit_frame(project_id, expected_revision,
  operation={type:"duplicate_frame", frame_id:"frame-1", new_frame_id:"rise", index:1})
apply_edits(project_id, expected_revision, operations=[
  {type:"clear_cel", frame_id:"rise", layer_id:"layer-1"},
  {type:"ellipse", frame_id:"rise", layer_id:"layer-1",
   x:9,y:10,width:6,height:9,palette_index:2,filled:true},
  {type:"set_frame_duration", frame_id:"rise", duration_ms:100}])
# Repeat with distinct apex/fall/landing poses, then:
get_contact_sheet(project_id, scale=8, columns=3)
inspect_project(project_id, frame_id="fall", scale=8) # targeted image review
apply_edits(project_id, expected_revision, operations=[...targeted correction...])
open_preview(project_id)                            # inspect local playback
finish_project(project_id)                          # editable source, no overwrite
# Export saved source with the existing CLI (below).
open_preview(path=".../bounce.gif")                 # user-facing delivery
```

`get_contact_sheet` returns actual PNG image content, visibly labelled with frame IDs and durations. Omit `frame_ids` for the ordered timeline, or supply an ordered unique subset. Defaults: scale 8, four columns; select 1–64 frames, maximum 16 million output pixels. Reduce scale/subset on a limit error. Metadata contains `revision`, original canvas `width`/`height`, sheet dimensions, each frame's ID, duration and `image_bounds`, and `distinct_composites`. Identical composited holds count once; a count of one warrants checking whether motion was actually drawn. Inspection is read-only and does not affect history or viewer selection. `get_preview` returns **only the first ordered frame**. Use `inspect_project` with an explicit stable ID for a particular frame. A contact sheet demonstrates poses; it does not prove playback. `open_preview` shows the user a window and supplies no image feedback to the AI. Do not claim visual playback verification unless it was actually observed.

Save/reopen preserves the editable timeline, layers, IDs, pixels and durations. The existing exporter renders every ordered frame for GIF/WebP/APNG unless `--frame` is specified; PNG and other static formats show only frame one by default. Animated exports play once by default; opt into indefinite looping:

```powershell
pixen-cli export bounce.pixen.json --output bounce --formats gif --loop
```

`--loop` requires at least two frames and a GIF/WebP/APNG output; it conflicts with `--frame`. Static formats in a mixed export remain static. GIF uses 10 ms cumulative endpoint rounding and palette quantization; alpha is thresholded to binary transparency unless an opaque `--background` is supplied. WebP uses 1 ms timing; APNG normally uses 0.1 ms, adapting precision for long holds. Durations that become zero or exceed format limits fail before output publication. Very short GIF delays may be clamped by third-party players. The viewer always loops its local playback and does not edit source data or inherit a file's finite loop count.

`open_preview(path=...)` supports saved projects and GIF through the existing importer. GIF has no editable layer structure/stable source IDs; its read-only preview contains decoded composites and durations. Source files are never modified. Missing/unsupported files and unavailable desktops return actionable errors. Closing a window leaves editing/server state intact.

The reproducible [six-pose bouncing-orb example](../../examples/mcp-animation/README.md) uses actual MCP calls for creation, frame duplication/durations, drawing, PNG review, correction and save. CLI export then reloads the saved source. It includes the editable project, labelled sheet, individual frames, GIF and tool transcript.


## Large AI drawing calls

Keep complete JSON tool calls below **200 KiB in UTF-8**, including the tool name, arguments and protocol envelope. Some AI clients reject calls around 256 KiB before they reach Pixen. This is a compatibility budget, not a Pixen transport limit. Instructions guide the model; they cannot enforce a provider's output size.

Prefer `rectangle`, `line`, `ellipse` and `fill` for geometry. For dense arbitrary pixels, use `set_pixel_rows` through `draw` or `apply_edits`:

```json
{"type":"set_pixel_rows","x":4,"y":6,"rows":[[1,2,1],[2,0,2]]}
```

This writes a 3 × 2 patch at (4,6), preserving everything outside it. All entries are palette indices; zero has whatever color palette entry zero defines. Empty, ragged, out-of-bounds patches and invalid indices reject the whole transaction. A 128 × 128 patch using single-digit indices takes about 33 KiB, compared with about 549 KiB for individual pixel objects. Multi-digit indices cost more.

Split larger patches into row strips within the budget. Wait for each response, then use its returned revision as the next `expected_revision`. Each batch is one undo step; a later failure leaves earlier successful batches intact. Never retry an oversized call unchanged. Crop pixel/image inspections and select relevant contact-sheet frames to avoid excessive response/context sizes.
