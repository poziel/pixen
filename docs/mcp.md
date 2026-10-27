# MCP and live canvas

Build and run from this repository (Rust 1.89 or newer):

```powershell
cargo build --locked --release
.\target\release\pixen-mcp.exe --viewer
# Headless:
.\target\release\pixen-mcp.exe
```

If a running MCP client has locked the existing Windows executable, build a
separate copy without stopping that client:

```powershell
cargo build --release --locked --target-dir target/mcp-editing
.\target\mcp-editing\release\pixen-mcp.exe --viewer
```

Point the client configuration below at that executable to use the new copy.
Restart the MCP client connection after changing the binary/configuration.
Check `tools/list`: this implementation exposes 30 tools including
`edit_frame`, `inspect_project`, `get_contact_sheet`, and `open_preview`.
A stale four-tool server can only expose the original static-preview workflow;
source changes alone do not upgrade a running process. On Windows the original
executable may remain locked by existing clients, so leave those clients intact
and select the separate build explicitly. For that build use:

```toml
[mcp_servers.pixen]
command = 'D:\Dev\5-tools\pixen\target\mcp-editing\release\pixen-mcp.exe'
args = ["--viewer"]
```


On Linux/macOS use `./target/release/pixen-mcp`. Linux needs the normal egui/eframe
X11/Wayland build and runtime libraries and a working OpenGL desktop. The optional viewer is a separate
process of the sibling `pixen` desktop executable; document edits always run inside the server.
The server uses the [official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)'s stdio transport. Stdout is reserved
for protocol messages; diagnostics go to stderr.

Example MCP client configuration (replace the executable path):

```json
{
  "mcpServers": {
    "pixen": {
      "command": "D:\\Dev\\5-tools\\pixen\\target\\release\\pixen-mcp.exe",
      "args": ["--viewer"]
    }
  }
}
```

For Codex `config.toml`:

```toml
[mcp_servers.pixen]
command = 'D:\Dev\5-tools\pixen\target\release\pixen-mcp.exe'
args = ["--viewer"]
```

Remove `--viewer` to avoid automatically opening a viewer on project creation;
explicit `open_preview` calls still open a window. Headless editing and PNG
inspection need no desktop. The client launches the executable; do not
start a second copy manually for the same connection. Use absolute output paths
to avoid depending on the client's working directory.

## Tool sequence

1. `create_project`:

   ```json
   {"width":16,"height":16,"palette":["#00000000","#2048A0FF","#70C8FFFF"],"output_path":"D:/Dev/5-tools/pixen/out/sprite.pixloom"}
   ```

   The parent directory must exist and the destination must be new. The response
   gives `project_id`, revision `0`, `frame_id` and `layer_id`. Palette indices
   remain stable; a transparent entry is appended if none was supplied.

2. `apply_edits` (use IDs from the response):

   ```json
   {"project_id":"project-1","expected_revision":0,"operations":[
     {"type":"rectangle","frame_id":"frame-1","layer_id":"layer-1","x":3,"y":4,"width":10,"height":8,"palette_index":1,"filled":true},
     {"type":"line","frame_id":"frame-1","layer_id":"layer-1","x1":4,"y1":5,"x2":11,"y2":5,"palette_index":2},
     {"type":"set_pixels","frame_id":"frame-1","layer_id":"layer-1","pixels":[{"x":6,"y":8,"palette_index":2},{"x":9,"y":8,"palette_index":2}]}
   ]}
   ```

3. `inspect_project` for AI image review:

   ```json
   {"project_id":"project-1","frame_id":"frame-1","scale":8,"crop":{"x":3,"y":4,"width":10,"height":8}}
   ```

   This returns actual PNG MCP image content and a text JSON block containing
   `project_id`, `revision`, `frame_id`, original canvas `width`/`height`, `scale`,
   and the `source` rectangle with its canvas coordinates. Omit `crop` for the
   full composite. Frame defaults to `frame-1`, scale to 8; crops must lie wholly
   inside the canvas and have positive dimensions. This call never changes the document or AI editing targets. While Follow AI
   is enabled it selects this frame and composite mode in the managed viewer.

4. `open_preview` for the user-facing window:

   ```json
   {"project_id":"project-1"}
   ```

   It opens or reuses, shows, restores and requests foreground focus for that
   project's preview. It returns after the viewer acknowledges initialization,
   without waiting for user approval or window closure. **Showing the user a
   window provides no visual feedback to the AI.** Use `inspect_project` for
   an actual image the AI can review.

   To preview an existing file instead, supply `path` and omit `project_id`:

   ```json
   {"path":"D:/Dev/5-tools/pixen/skills/pixen/examples/animated.pixloom.json"}
   ```

   Supported inputs use Pixen's existing loaders: authoring JSON, `.pixloom`
   (and legacy Spritex inputs), `.pxo`, `.ase`, and `.aseprite`, with the same
   supported-feature restrictions as native import. Files are never written.
   Canonical paths identify previews; repeated calls reuse the same process.
   Missing, malformed or unsupported files produce actionable tool errors.

   Success includes `opened: true`, `reused`, `pid`, `activation_requested`,
   observed `focused`, `attention_requested`, dimensions and `frame_count`.
   Active projects include their revision; existing-file previews include the
   canonical path and `revision: null` because they have no editing session.
   Failures set MCP `isError` and return `opened: false` with an `error` message.
   Opening/re-activation waits at most three seconds for acknowledgement per
   attempt, plus bounded process cleanup. Disconnected windows are reaped and
   replaced; a timeout leaves an actionable error for retry.

5. Edit again using `expected_revision: 1`, then call `finish_project` with
   `{"project_id":"project-1"}`. Successful finish saves validated Pixen
   authoring JSON, closes the viewer, and allows another project on the same
   connection. Existing files are never overwritten. Save failure keeps the
   project and viewer available for retry.

`get_preview` remains backward-compatible: `{"project_id":"project-1","scale":8}`
returns the first-frame composite PNG and its original revision/dimensions.
There are 30 tools. `apply_edits` remains the atomic batch-edit path;
the focused mutation tools below use the same typed executor.

All coordinates are zero-based and must be inside the canvas. Rectangles use
origin and positive dimensions; lines include both endpoints. `fill` takes
`x`, `y`, `palette_index` and replaces four-connected pixels with the same
palette index. `clear` takes `palette_index` and fills the targeted cel.
Both also require `frame_id` and `layer_id`. Batches execute in order on a
candidate document and commit together. A successful batch increments revision
once, even when it changes no pixels. `changed_bounds` encloses the actual
changed indices, or is null for no change. Drawing-only batches also return a
`changes` list with per-frame/layer bounds, including explicitly linked cels.
Structural edits and undo/redo conservatively return the full canvas bounds.

## Inspection and editing tools

| Tool | Purpose |
| --- | --- |
| `create_project` | Start one transparent project; prefer a `.pixloom` destination |
| `open_project` | Open any supported inspector input for editing; default destination is `.pixloom` |
| `save_project` | Atomic checkpoint without ending the session |
| `export_project` | Explicit native-project/image export without changing the project destination |
| `edit_palette` | Append/update exact colors or explicitly remap indices |
| `copy_region`, `transform_region` | Snapshot-based copying, translation, flips and clockwise quarter-turns |
| `get_onion_skin`, `compare_frames`, `inspect_layer` | Actual PNG review and matching Follow AI inspector mode |
| `report_activity` | Describe current work without changing artwork |
| `get_review_sheet` | Multi-scale color/silhouette/grayscale images |
| `manage_checkpoint`, `compare_checkpoint` | Independent immutable baselines and before/after evidence |
| `analyze_sprite` | Geometry, colors and annotated component measurements |
| `validate_artwork` | Explicit caller rules, including protected regions |
| `audit_animation` | Timing, holds, pose/transition and optional loop evidence |
| `get_project_info` | Dimensions, palette, ordered layer/frame IDs, durations, links and revision |
| `get_pixels` | At most 4096 exact pixels from an explicit layer/frame region |
| `edit_layer` | One typed layer operation |
| `edit_frame` | One typed frame operation |
| `draw` | One typed drawing operation on an explicit layer/frame |
| `apply_edits` | Mix typed drawing, layer and frame operations in one transaction |
| `undo`, `redo` | Restore one whole transaction, with revision checks |
| `inspect_project`, `get_preview`, `get_contact_sheet` | Explicit actual PNG review; no automatic mutation images |
| `open_preview` | Open/focus a read-only user preview |
| `finish_project` | Save, close the managed viewer and release the session; refuse external file changes |

Inspection uses `{"project_id":"project-1"}`. `get_project_info` returns
zero-based ordered `index` values, stable `id` fields, exact `#RRGGBBAA` palette
entries, layers with visibility/opacity/lock state and cel/link timelines, frames
with durations, and history availability. Frame IDs are persisted in each
frame's `annotations.metadata.pixloom_frame_id`. Reorder never changes an ID.
Caller-supplied new frame/layer IDs must satisfy Pixen's identifier rules and
must never have been used in that project, including IDs removed or undone.
Failed transactions do not reserve IDs.

For exact region inspection:

```json
{"project_id":"project-1","frame_id":"frame-1","layer_id":"layer-1","region":{"x":0,"y":0,"width":4,"height":4}}
```

`get_pixels` returns row-major `pixels` entries containing explicit source `x`,
`y`, zero-based `palette_index`, and exact stored `rgba` strings. It reports raw
cel pixels, including transparent RGB values and duplicate palette indices;
layer visibility/opacity and cel opacity are not applied. Use the PNG tool for
the composited result. Empty cels report the transparent palette index.

Focused mutations take `project_id`, `expected_revision`, and an `operation`.
`draw` also requires explicit `frame_id` and `layer_id`. In `apply_edits`, each
drawing operation includes those targets directly alongside its `type` and
fields. Layer operations carry a `layer_id`; frame operations carry a `frame_id`.
All paths run through one candidate-document executor and one validation/save
boundary; no viewer state is consulted.

| Layer operation `type` | Fields beyond `type` |
| --- | --- |
| `add_layer` | New `layer_id`, nonblank `name`, optional insertion `index` |
| `remove_layer` | `layer_id` |
| `rename_layer` | `layer_id`, nonblank `name` |
| `reorder_layer` | `layer_id`, final `index` |
| `update_layer` | `layer_id`, `visible` and/or `opacity` in 0..1 |

Layer index 0 is the bottom of the composite. New layers contain transparent
cels on every frame. Removing a layer removes its timeline; the last layer
cannot be removed. Drawing on locked layers is rejected.

| Frame operation `type` | Fields beyond `type` |
| --- | --- |
| `add_frame` | New `frame_id`, positive finite `duration_ms`, optional insertion `index` |
| `remove_frame` | `frame_id` |
| `duplicate_frame` | Source `frame_id`, new `new_frame_id`, optional insertion `index` |
| `reorder_frame` | `frame_id`, final `index` |
| `set_frame_duration` | `frame_id`, positive finite `duration_ms` |

Frame index 0 is the first frame. Additions default to append; duplicate defaults
to immediately after its source. Insertion indices are 0..count; reorder indices
are 0..count-1 and describe the final position. New frames are transparent.
Duplication copies all source layer pixels, duration, annotations and cel
opacity, allocates independent cel storage, and clears the duplicate's explicit
link groups. Editing either copy cannot change the other. The original's links
remain intact. The last frame cannot be removed. Tags retain their existing
frame membership; edits that make a tag non-contiguous are rejected, and tags
with no remaining members disappear. Unreferenced cel definitions are retained;
this slice does not introduce cel compaction.

Drawing operations:

| `type` | Fields beyond explicit targets |
| --- | --- |
| `set_pixels` | `pixels: [{x,y,palette_index}, ...]`; later entries win at repeated coordinates |
| `set_pixel_rows` | `x, y, rows: [[palette_index, ...], ...]`; dense rectangular patch with equal-width non-empty rows, strict bounds; every entry overwrites, including transparent indices |
| `line` | `x1,y1,x2,y2,palette_index`; Bresenham, including both endpoints |
| `rectangle` | `x,y,width,height,palette_index,filled` |
| `ellipse` | `x,y,width,height,palette_index,filled` |
| `fill` | `x,y,palette_index`; four-connected replacement of equal indices |
| `erase_region` | `x,y,width,height`; replace the entire rectangle with transparency |
| `clear_cel` | No colour fields; replace the whole target cel with transparency |
| `clear` | Legacy whole-cel fill using explicit `palette_index` |

Coordinates are integer, zero-based, and strictly bounded. Rectangles and ellipse
bounds have positive dimensions and cover `x..x+width-1`, `y..y+height-1`.
Rectangle outlines include their border pixels. Ellipses include pixel centres
inside the geometric ellipse centred in the bounding rectangle, with radii
`width/2` and `height/2`. An outlined ellipse keeps included pixels with at least
one four-connected neighbour outside the ellipse. Thin one-pixel shapes are
valid. No operation clips silently; even a partially out-of-bounds shape or
region rejects the complete transaction. Fill compares exact palette indices,
not equivalent RGBA colours. Erase chooses the first alpha-zero palette entry
(appending `#00000000` only when needed). Other colours must already exist in
the palette; indices never renumber. Explicit layer-local link groups share
intentional edits; unlinked reused cel storage detaches before writing.

### Compact workflow

After `create_project`, inspect with `get_project_info`, then:

```json
{"project_id":"project-1","expected_revision":0,"operation":{"type":"add_layer","layer_id":"detail","name":"Detail"}}
```

Call that through `edit_layer`, then add a frame through `edit_frame`:

```json
{"project_id":"project-1","expected_revision":1,"operation":{"type":"add_frame","frame_id":"pose-2","duration_ms":150}}
```

Draw through `apply_edits` (using the palette from the creation example):

```json
{"project_id":"project-1","expected_revision":2,"operations":[
  {"type":"ellipse","frame_id":"pose-2","layer_id":"detail","x":3,"y":3,"width":10,"height":10,"palette_index":1,"filled":true},
  {"type":"line","frame_id":"pose-2","layer_id":"detail","x1":5,"y1":6,"x2":10,"y2":6,"palette_index":2}
]}
```

Review the actual image with `inspect_project`:
`{"project_id":"project-1","frame_id":"pose-2","scale":8}`.
Make a targeted correction through `draw`:

```json
{"project_id":"project-1","expected_revision":3,"frame_id":"pose-2","layer_id":"detail","operation":{"type":"set_pixels","pixels":[{"x":7,"y":8,"palette_index":2}]}}
```

Then `open_preview` with `{"project_id":"project-1"}`. The viewer can inspect
either frame without changing any AI target. No mutation generates a PNG unless
the client explicitly calls an image-preview tool.

### Transactions, undo and errors

Every mutation requires `expected_revision`. It validates the complete candidate
document and writes recovery before publishing any changes. Invalid IDs,
coordinates, palette indices, opacity, duration, limits, stale revisions and
save failures leave document, revision and history unchanged. Tool errors use
MCP `isError`; malformed argument schemas are rejected by the SDK before editing.
Existing `open_preview` errors additionally report `opened: false`.

`undo` and `redo` take `{"project_id":"project-1","expected_revision":4}`.
One successful batch is one undo step, including no-op batches. Revisions always
increase, including undo/redo; clients must use the returned revision for their
next call. A new successful mutation after undo clears redo. History stores at
most 32 document snapshots and 64 MiB total, evicting the oldest available steps.
Each editing document must serialize within 64 MiB to participate. History is
in-memory only and disappears on finish/server shutdown; recovery retains the
latest document. Removing frames/layers and clearing/erasing pixels are
destructive operations, undoable only while their transaction remains in history.
No confirmation dialog is added to an explicitly requested mutation.

## Viewer and recovery

The egui/eframe app keeps document ownership in the MCP server. A horizontal
frame-thumbnail strip sits directly beneath the checkerboard/composited canvas;
playback controls and the current-frame indicator sit below the strip. The
selected thumbnail has a highlighted border. Overflow adds previous/next frame
arrows and horizontal scrolling; selection follows into view immediately.
Thumbnails use the same renderer and nearest-neighbour filtering, with lazy
bounded-size texture caching. Static projects show a single thumbnail and omit
playback/navigation controls. Playback starts looping and respects each frame's
duration. Selecting a frame pauses; Stop pauses and returns to frame one.
Space toggles playback, Home stops, and Left/Right select frames. All viewing
state is local and read-only; it never changes document pixels, frame durations,
or the AI's editing selection.

The viewer polls its source every 100 ms. Active editing projects retain the
existing recovery-snapshot update flow; existing-file previews reload through
the shared input/native loaders. Session frame IDs keep local selection stable
across reorder; removal clamps the local selection. Reloads
reset elapsed timing; removing all but one frame disables playback. A failed
reload preserves the last valid display and shows an actionable source error.
Restoring a valid source resumes updates. Rendering uses the same composite
renderer as AI PNG previews and exports, with integer nearest-neighbour scaling
in physical pixels. Smaller-than-1x viewports clip the centred canvas. Resizing
never changes the document.

Explicit open requests use the framework's supported Visible, Minimized and
Focus commands. Normal live updates never issue activation commands. The window
has normal stacking order and never becomes always-on-top. Focus is a request,
not a guarantee: OS policy may deny it, and winit's direct focus call is not
supported on Wayland. If focus is not observed, the app makes one supported
informational attention request. There is no input injection, topmost toggle,
or focus-stealing workaround in the product. These restrictions are documented
by [winit's window API](https://docs.rs/winit/0.30.13/winit/window/struct.Window.html#method.focus_window).

Closing a preview leaves its project and MCP connection intact. `open_preview`
can reopen it. Successful finish closes only that editing project's preview;
other file previews remain open until closed or until the MCP server ends.
Viewer startup errors are reported through the tool and stderr. Automatic
`--viewer` startup failures leave editing available headlessly. An interactive
desktop and working OpenGL backend are required to open windows.

Creation and every accepted edit persist a validated `.pixloom-recovery-*.pixloom.json`
beside the destination before acknowledging success. The snapshot path is
printed to stderr. Unexpected shutdown leaves this recoverable authoring file;
it does not create the final output or mark the session finished. Copy the
snapshot to a new `.pixloom.json` path, or call `open_project` on it with an
explicit `.pixloom` output_path to resume through MCP. Automatic session restore is not implemented.
Successful finish removes the recovery snapshot. A crash after final-file
publication but before cleanup may leave both files; the saved final file is
valid. Recovery contains the document, not protocol IDs or revision history.

Editing supports one active project with multiple frames and layers, within
Pixen's document limits. File viewing supports the existing formats' static
and animated projects. Preview images are limited to 16 million pixels; batches
to 10,000 operations. There is
no human drawing editor, freeform selection masks, automatic tweening, or collaboration. Retaining a manual-inspection window after the MCP
connection ends is not implemented.

## Verification

```powershell
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
# Only the stdio smoke test:
cargo test --locked --test mcp_protocol
# Interactive Windows desktop: opens real windows and samples their pixels:
cargo test --locked --test mcp_protocol graphical -- --ignored --nocapture --test-threads=1
```

The protocol tests launch the real executable, discover exactly 30 tools,
create and draw a multicolour sprite, decode the returned PNG and check its
pixels and dimensions, edit and finish, then start another project and
check that forced shutdown preserves a valid recovery document. They also check
the inspection PNG/crop contract, invalid frame/crop/scale errors, and the
user-preview error contract. Session tests
cover atomic rejection, stale revisions, invalid coordinates and targets,
rectangle dimensions, palette validation, four-connected fill, no-op fill,
clear, line drawing and save failures. Preview tests compare crops against the
shared composite renderer, check file bytes and modification times remain
unchanged, load native/container inputs, and exercise unequal frame durations,
looping, pause/selection, stop, large time steps and frame additions/removals.
Checkerboard composition has a unit test. These tests do not establish image
display inside an MCP host.

The opt-in Windows tests require an interactive desktop, run serially, and
sample screen pixels across static live updates. They check restore/reuse,
normal stacking style, no foreground change on ordinary edits, resize, failed
save retention, manual close and reopen, non-mutating file preview, independent
window lifetimes, and shutdown recovery. Run them outside an isolated desktop
sandbox; screen sampling is not meaningful on a hidden desktop.

The previous window slice was observed manually on Windows. This thumbnail and
editing increment adds headless egui layout/input tests for actual thumbnail
images, strip/control order, overflow arrows, selected-frame scrolling, click
and keyboard selection, and static control omission. Core/protocol tests cover
layer/frame ordering, independent duplication, cel/link semantics, geometry,
strict bounds, region inspection, atomic rollback, stale revisions, history
limits, redo invalidation, save failures and viewer-selection independence.
See the animation example verification record for the current Windows visual checks.
MCP-host image display and Linux/macOS/Wayland desktop behaviour remain unverified.


## Authoring and delivering animation

An animation request means multiple **visually distinct frames with intentional
motion**, not just a static sprite saved in an animation-capable project. Frame
creation/duplication does not invent motion or tween pixels. Duplicate frames
are useful holds or starting points; draw the changed poses explicitly.

Recommended sequence: **block out → review → create keyframes → add useful
intermediates → inspect contact sheet and playback → correct → save/export →
open user preview**. Keep one revision-checked `apply_edits` batch per logical
transaction. `edit_frame` uses that same executor. `add_frame` appends by default;
`duplicate_frame` inserts immediately after its source by default. Specify
zero-based `index` explicitly when assembling an ordered timeline. Duplication
copies all layers into independent cels; subsequent drawing cannot alter its
source. Reordering preserves stable IDs. Durations are positive finite ms.

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

`get_contact_sheet` returns actual PNG image content, visibly labelled with frame
IDs and durations. Omit `frame_ids` for the ordered timeline, or supply an ordered
unique subset. Defaults: scale 8, four columns; select 1–64 frames, maximum
16 million output pixels. Reduce scale/subset on a limit error. Metadata contains
`revision`, original canvas `width`/`height`, sheet dimensions, each frame's ID,
duration and `image_bounds`, and `distinct_composites`. Identical composited holds
count once; a count of one warrants checking whether motion was actually drawn.
Inspection is read-only and does not affect history or viewer selection.
`get_preview` returns **only the first ordered frame**. Use `inspect_project` with
an explicit stable ID for a particular frame. A contact sheet demonstrates poses;
it does not prove playback. `open_preview` shows the user a window and supplies
no image feedback to the AI. Do not claim visual playback verification unless
it was actually observed.

Save/reopen preserves the editable timeline, layers, IDs, pixels and durations.
The existing exporter renders every ordered frame for GIF/WebP/APNG unless
`--frame` is specified; PNG and other static formats show only frame one by
default. Animated exports play once by default; opt into indefinite looping:

```powershell
pixen-cli export bounce.pixloom.json --output bounce --formats gif --loop
```

`--loop` requires at least two frames and a GIF/WebP/APNG output; it conflicts
with `--frame`. Static formats in a mixed export remain static. GIF uses 10 ms
cumulative endpoint rounding and palette quantization; alpha must be binary or
supply an opaque `--background`. WebP uses 1 ms timing; APNG normally uses 0.1 ms,
adapting precision for long holds. Durations that become zero or exceed format
limits fail before output publication. Very short GIF delays may be clamped by
third-party players. The viewer always loops its local playback and does not
edit source data or inherit a file's finite loop count.

`open_preview(path=...)` supports saved projects and GIF through the existing
importer. GIF has no editable layer structure/stable source IDs; its read-only
preview contains decoded composites and durations. Source files are never
modified. Missing/unsupported files and unavailable desktops return actionable
errors. Closing a window leaves editing/server state intact.

The reproducible [six-pose bouncing-orb example](../examples/mcp-animation/README.md)
uses actual MCP calls for creation, frame duplication/durations, drawing, PNG
review, correction and save. CLI export then reloads the saved source. It includes
the editable project, labelled sheet, individual frames, GIF and tool transcript.

## Desktop, CLI and MCP executable names

`pixen` is the desktop app, `pixen-cli` runs file commands, and `pixen-mcp`
runs the stdio MCP server. Keep all three executables in the same directory and
ship them at matching versions. `open_preview` launches the sibling `pixen`
app with its managed stdio lifecycle. MCP diagnostics go to stderr; stdout is
reserved for protocol traffic.

Point new MCP configurations at `pixen-mcp.exe` with `args = ["--viewer"]`
(or no arguments for headless editing). The CLI has no MCP subcommand;
`pixen-mcp` is the dedicated server entry point.
Launching `pixen` without arguments opens an empty viewer; a project argument,
Open, or drag-and-drop loads a file through the shared library.


### Live viewer activity and following

Managed previews show an activity strip for MCP drawing, layer/frame edits,
and undo/redo, with completion or failure status. It reports actual
tool operations, not the model's private reasoning. Mixed edit batches show
their operation count and follow the last explicit frame/layer targets.

**Follow AI** uses an on/off switch to select the editing frame and layer in the viewer.
While following, manual controls and canvas navigation are disabled; the switch
remains available. Turn it off
to inspect another selection while live document updates continue. Following
does not change the MCP editing targets or saved layer visibility. Layer eye
controls and “show only this layer” remain local preview overrides.

Use `report_activity` for your own concise activity descriptions:

```json
{"project_id":"project-1","message":"Drawing the ranger's weapons","busy":true,"frame_id":"frame-1","layer_id":"layer-1"}
```

The description and spinner stay active through subsequent drawing/layer/frame
edits. Call again with `busy:false` and a completion message to finish the
activity and restore automatic descriptions. Messages are limited to 240
characters on a single line. Reporting never edits the document or revision.


## Project open, save and export

Use `.pixloom` as the normal MCP/app project format; JSON remains supported for
direct authoring and debugging. Recovery snapshots remain internal authoring JSON.
`create_project.output_path` accepts `.pixloom` or explicit `.pixloom.json`.

`open_project` takes `path` and optional `output_path`. It accepts all inspector
inputs: Pixen JSON/containers (including legacy inputs), `.ase`/`.aseprite`,
`.pxo`, PNG/JPEG/GIF/APNG/WebP/BMP/TIFF. Existing supported-feature restrictions
apply; unsupported native features are rejected rather than silently discarded.
Authoring inputs retain exact palette indices and persisted frame IDs. Native
and raster imports get an editable indexed document with stable frame IDs.

Opening `.pixloom` defaults to saving that same file. Other inputs default to a
sibling `.pixloom` destination, preserving their original files. Existing sibling
destinations require a different `output_path`. `open_project` returns full project
info including `output_path`; query `get_project_info` after creation for that path.

`save_project({project_id})` checkpoints the chosen destination and keeps revision,
history and the viewer. `finish_project` saves and ends the session. Saving an
opened or previously saved file refuses external disk changes, including deletion;
failure retains the active project/recovery. Save operations stage output before
publication; `.pixloom` containers are reopened and fully validated before commit.

`export_project` takes `project_id`, `output_path`, optional `overwrite` (false),
`frame_id` and `loop_animation` (false). The extension selects `.pixloom`,
`.pixloom.json`, `.ase`/`.aseprite`, `.pxo`, or an image format supported by the CLI.
Native/project export preserves the whole timeline and rejects frame/loop options.
Image `frame_id` is a stable ID; omit it for the full animated timeline or the
first frame for static formats. Use the CLI for video and explicit image
background options. Export cannot replace the active destination or recovery;
use `save_project` for project updates. Exporting to another application's source
file requires an explicit export request and `overwrite:true` if it exists.

## Palette and region edits

New operations also work in `apply_edits` batches and share revision checks,
validation, recovery, locked-layer rules and undo/redo with drawing.

`edit_palette` takes `project_id`, `expected_revision` and `operation`:

- `{"type":"append_colors","colors":["#AABBCCFF"]}` appends without deduplication.
- `{"type":"set_color","palette_index":1,"color":"#123456FF"}` changes an
  entry globally. Other indices never shift. If needed, a transparent entry is
  appended so empty cels remain editable.
- `{"type":"remap_color","from_index":1,"to_index":2,"frame_id":"pose-2","layer_id":"ink"}`
  replaces exact indices; omit either scope for all frames/layers. Equal RGBA at
  another index is untouched. Intentional cel links still share edits, including
  outside the selected frame; unlinked storage detaches. Locked affected layers
  reject the transaction. Palette entry updates are explicitly global, including
  colors used by locked layers.

`copy_region` takes `project_id`, `expected_revision`, `source_frame_id`,
`source_layer_id`, `region:{x,y,width,height}`, destination `frame_id`, `layer_id`,
`x`, `y`, and optional `transform`. Source is read before writing so overlapping
copies are deterministic. Transparent pixels replace destination pixels too.

`transform_region` takes the same project/revision, `frame_id`, `layer_id`,
`region`, destination `x`,`y`, and required `transform`. It reads the source,
clears the original rectangle, then pastes the transformed pixels in one undoable
transaction. Transforms are `none` (translation), `flip_horizontal`,
`flip_vertical`, `rotate90`, `rotate180`, `rotate270`; rotations are clockwise.
Quarter-turns swap output width/height. Both source and destination must fit wholly
inside the canvas; no clipping or resampling occurs. In `apply_edits`, use
`type:"copy_region"` or `type:"transform_region"` alongside those fields.

## Onion skin, comparison and isolated layers

`get_onion_skin`, `compare_frames` and `inspect_layer` take `project_id`, explicit
`frame_id`, optional `reference_frame_id`, optional `layer_id` and `scale` (8).
`inspect_layer` requires `layer_id` and forces that layer visible in the preview.
Other tools may isolate a layer in the same way. Stored visibility is unchanged.

Onion skin places current artwork over faint red previous and cyan next poses,
without wrapping the timeline. An explicit reference replaces both neighbors
with one red reference. Frame comparison defaults to the previous frame (self on
the first frame), marks every changed RGBA pixel pink, and dims unchanged pixels.
It returns `changed_pixels`, counting additions, removals and alpha changes once.
This measures pixel differences, not artistic quality or whether a loop is good.
All tools return PNG MCP image content and revision/dimension/frame metadata;
none changes document revision or undo history.

The inspector's existing menu includes **Onion skin** and **Frame comparison**
toggles and a reference-frame selector. These are mutually exclusive inspection
modes. Manual mode changes pause playback. An isolated layer is identified in the
menu with **Show all layers** to restore the composite.

While **Follow AI** is on, successful inspection calls select their frames,
reference, isolated layer and mode in the managed viewer. The menu remains
openable to show the active settings, with its controls disabled while following.
`inspect_project` restores composite mode. AI-selected views reset local layer
visibility overrides so the image agrees with the AI render. While following is
off, AI requests do not change your view; re-enabling Follow AI adopts the latest
request. Viewer settings never change saved artwork or explicit editing targets.


See [art-review APIs](art-review.md) for all six review schemas, resource limits,
inspector controls and examples. [Future critic workflow](critic-workflow.md) is
a design document only; no critic agent or skill is installed or run.

### Resize canvas bounds

`resize_canvas` is available in both standalone MCP and the embedded artist's
active-document tool set. Pass `project_id`, `expected_revision`, `width`, and
`height`. Pixels stay at the top left; smaller bounds crop every cel, larger
bounds pad with transparency. Palette indices, layer/frame IDs, and timing are
preserved. A successful change advances revision once and is one undo step;
stale revisions and invalid dimensions leave the project unchanged.

```json
{"name":"resize_canvas","arguments":{"project_id":"project-1","expected_revision":3,"width":64,"height":48}}
```

## Large AI drawing calls

Keep complete JSON tool calls below **200 KiB in UTF-8**, including the tool name,
arguments and protocol envelope. Some AI clients reject calls around 256 KiB
before they reach Pixen. This is a compatibility budget, not a Pixen transport
limit. Instructions guide the model; they cannot enforce a provider's output size.

Prefer `rectangle`, `line`, `ellipse` and `fill` for geometry. For dense arbitrary
pixels, use `set_pixel_rows` through `draw` or `apply_edits`:

```json
{"type":"set_pixel_rows","x":4,"y":6,"rows":[[1,2,1],[2,0,2]]}
```

This writes a 3 × 2 patch at (4,6), preserving everything outside it. All entries
are palette indices; zero has whatever color palette entry zero defines. Empty,
ragged, out-of-bounds patches and invalid indices reject the whole transaction.
A 128 × 128 patch using single-digit indices takes about 33 KiB, compared with
about 549 KiB for individual pixel objects. Multi-digit indices cost more.

Split larger patches into row strips within the budget. Wait for each response,
then use its returned revision as the next `expected_revision`. Each batch is one
undo step; a later failure leaves earlier successful batches intact. Never retry
an oversized call unchanged. Crop pixel/image inspections and select relevant
contact-sheet frames to avoid excessive response/context sizes.
