# MCP and live canvas

Build and run from this repository (Rust 1.89 or newer):

```powershell
cargo build --locked --release
.\target\release\pixloom.exe mcp --viewer
# Headless:
.\target\release\pixloom.exe mcp
```

If a running MCP client has locked the existing Windows executable, build a
separate copy without stopping that client:

```powershell
cargo build --release --locked --target-dir target/mcp-editing
.\target\mcp-editing\release\pixloom.exe mcp --viewer
```

Point the client configuration below at that executable to use the new copy.

On Linux/macOS use `./target/release/pixloom`. Linux needs the normal egui/eframe
X11/Wayland build and runtime libraries and a working OpenGL desktop. The optional viewer is a separate
process of this same executable; document edits always run inside the server.
The server uses the [official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)'s stdio transport. Stdout is reserved
for protocol messages; diagnostics go to stderr.

Example MCP client configuration (replace the executable path):

```json
{
  "mcpServers": {
    "pixloom": {
      "command": "D:\\Dev\\5-tools\\pixloom\\target\\release\\pixloom.exe",
      "args": ["mcp", "--viewer"]
    }
  }
}
```

For Codex `config.toml`:

```toml
[mcp_servers.pixloom]
command = 'D:\Dev\5-tools\pixloom\target\release\pixloom.exe'
args = ["mcp", "--viewer"]
```

Remove `--viewer` to avoid automatically opening a viewer on project creation;
explicit `open_preview` calls still open a window. Headless editing and PNG
inspection need no desktop. The client launches the executable; do not
start a second copy manually for the same connection. Use absolute output paths
to avoid depending on the client's working directory.

## Tool sequence

1. `create_project`:

   ```json
   {"width":16,"height":16,"palette":["#00000000","#2048A0FF","#70C8FFFF"],"output_path":"D:/Dev/5-tools/pixloom/out/sprite.pixloom.json"}
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
   inside the canvas and have positive dimensions. This call never changes the
   document, AI editing selection, or local viewer frame.

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
   {"path":"D:/Dev/5-tools/pixloom/skills/pixloom/examples/animated.pixloom.json"}
   ```

   Supported inputs use Pixloom's existing loaders: authoring JSON, `.pixloom`
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
   `{"project_id":"project-1"}`. Successful finish saves validated Pixloom
   authoring JSON, closes the viewer, and allows another project on the same
   connection. Existing files are never overwritten. Save failure keeps the
   project and viewer available for retry.

`get_preview` remains backward-compatible: `{"project_id":"project-1","scale":8}`
returns the first-frame composite PNG and its original revision/dimensions.
There are thirteen tools. `apply_edits` remains the atomic batch-edit path;
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
| `create_project` | Start one transparent project with an initial frame/layer |
| `get_project_info` | Dimensions, palette, ordered layer/frame IDs, durations, links and revision |
| `get_pixels` | At most 4096 exact pixels from an explicit layer/frame region |
| `edit_layer` | One typed layer operation |
| `edit_frame` | One typed frame operation |
| `draw` | One typed drawing operation on an explicit layer/frame |
| `apply_edits` | Mix typed drawing, layer and frame operations in one transaction |
| `undo`, `redo` | Restore one whole transaction, with revision checks |
| `inspect_project`, `get_preview` | Explicit actual PNG review; no automatic mutation images |
| `open_preview` | Open/focus a read-only user preview |
| `finish_project` | Validate/save without overwriting existing files |

Inspection uses `{"project_id":"project-1"}`. `get_project_info` returns
zero-based ordered `index` values, stable `id` fields, exact `#RRGGBBAA` palette
entries, layers with visibility/opacity/lock state and cel/link timelines, frames
with durations, and history availability. Frame IDs are persisted in each
frame's `annotations.metadata.pixloom_frame_id`. Reorder never changes an ID.
Caller-supplied new frame/layer IDs must satisfy Pixloom's identifier rules and
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
snapshot to a new `.pixloom.json` path and use normal Pixloom commands to resume
working with it outside MCP. Automatic MCP session restore is not implemented.
Successful finish removes the recovery snapshot. A crash after final-file
publication but before cleanup may leave both files; the saved final file is
valid. Recovery contains the document, not protocol IDs or revision history.

Editing supports one active project with multiple frames and layers, within
Pixloom's document limits. File viewing supports the existing formats' static
and animated projects. Preview images are limited to 16 million pixels; batches
to 10,000 operations. There is
no human drawing editor, selections/transforms, collaboration, palette-editing
tool, or export tool. Retaining a manual-inspection window after the MCP
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

The protocol tests launch the real executable, discover exactly thirteen tools,
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
No new manual visual or MCP-host image-display verification is claimed for this
increment. Linux/macOS/Wayland desktop behaviour remains unverified.
