# MCP animation example: bouncing orb

A six-pose, 24×24 looping animation, created through actual stdio MCP calls.
The editable source has separate Ground and Orb layers. The pose order and
holds are intentional:

| Frame ID | Duration | Pose |
| --- | ---: | --- |
| frame-1 | 200 ms | Contact squash |
| rise | 100 ms | Upward stretch |
| apex-in | 140 ms | Approaching apex |
| apex-out | 220 ms | Apex hold |
| fall | 90 ms | Downward stretch |
| land | 120 ms | Landing transition |

One loop is 870 ms. Every composite differs. The falling pose receives a
frame-targeted correction after the first contact-sheet review.

- [Editable project](output/bounce.pixloom.json)
- [Looping GIF](output/bounce.gif)
- [Labelled contact sheet](output/contact-sheet.png)
- [MCP tool transcript](output/tool-transcript.json)
- [Final inspection metadata](output/review-metadata.json)

Individual frame PNGs and the initial blockout review are also in `output/`.
The transcript records this run's absolute paths; regeneration uses your chosen
fresh output directory. This example is separate from existing user artwork.

## Reproduce

From the repository root, with Rust 1.89+ and Python 3:

```powershell
cargo build --locked
python examples/mcp-animation/create.py --pixen target/debug/pixen-cli.exe --output out/my-animation --preview
```

Choose a new output directory; the script refuses to replace a saved project.
Omit `--preview` for fully headless generation. With `--preview`, the MCP
connection stays open until Ctrl+C, keeping both saved-project and GIF previews
available. Closing either window does not stop the server. Creation and every
edit use MCP; the existing CLI is invoked once to validate and once to export
the saved source, which reloads it from disk:

```powershell
.\target\debug\pixen-cli.exe export examples/mcp-animation/output/bounce.pixloom.json --output out/orb-loop --formats gif --loop
```

For MCP client configuration and the compact authoring sequence see
[the MCP documentation](../../docs/mcp.md#authoring-and-delivering-animation).
`get_contact_sheet` and `inspect_project` return actual PNG image content;
`open_preview` itself does not give image feedback to the AI.

## Verification performed on Windows, 2026-10-18

- Created the source using `create_project`, `edit_frame`, mixed `apply_edits`,
  `inspect_project`, `get_contact_sheet`, `get_project_info` and `finish_project`.
- Reviewed the actual returned labelled PNG, including all six poses and holds.
  This caught reversed insertion order in the example script; explicit indices
  corrected it before delivery.
- Validated and reopened the saved two-layer source, then exported its full
  timeline using Pixen's existing GIF encoder with infinite-loop metadata.
- Opened both saved source and GIF using MCP `open_preview`; both reported six
  frames and successful desktop initialization/focus. Reopening reused the
  existing source window and requested focus.
- Observed changing poses in both actual windows, including continued playback
  across multiple 870 ms cycles. Verified Play, Pause, Stop-to-first-frame,
  thumbnail selection/pause, the selected-frame indicator, crisp scaling,
  checkerboard transparency, thumbnails immediately beneath the canvas, and
  controls below the strip.
- Reproduced the entire MCP sequence with the optimized isolated release; the
  saved project, GIF and contact sheet matched the delivered files byte-for-byte.
- Protocol regression tests verify labelled image content, subset order,
  non-mutation, invalid arguments, per-frame review, save/reopen pixels and
  durations, and GIF reopening. Export tests decode GIF/WebP/APNG and check all
  frames, timing and loop metadata, retaining default one-play behaviour.
- `tests/animation_example.rs` independently reloads these delivered files and
  compares all six composites and durations exactly, checks layers/stable IDs,
  infinite GIF looping metadata, and non-mutating GIF loading.
- Switched to the repository File Explorer window successfully; the preview
  did not stay above it. Explicit MCP reopening then reused/focused the GIF.
- Both opt-in interactive Windows tests passed serially: live updates, window
  restoration/reuse, non-topmost style, no ordinary-edit focus activation,
  resizing, failed-save retention, manual closure/reopening, source-file
  preservation and shutdown recovery.

Exact timing was checked in playback logic and decoded files; desktop frame
presentation was observed rather than measured. This does not establish image
display inside a third-party MCP host. Linux/macOS/Wayland desktop behaviour and
external GIF players were not manually verified. GIF timing has 10 ms precision;
this example uses exact multiples of 10 ms. No automatic tweening is implemented.


## Runtime mismatch found during investigation

The repository already contained frame editing and local animation playback,
but the running MCP clients used the former CLI executable `target/release/pixen.exe`. Protocol discovery
confirmed that binary exposed only the original four tools. The older separate
`target/mcp-editing` build exposed thirteen; the updated implementation exposes
fourteen with `get_contact_sheet`. Also, `get_preview` intentionally returns a
single first-frame PNG and default animated exports play once. These are
concrete capability/delivery pitfalls; without inspecting the original artwork
we cannot establish which caused that particular static result. Configure and
restart the client against the updated isolated release as documented in
`docs/mcp.md`; a running server does not acquire newly built tools automatically.

The creation script uses the sibling `pixen-mcp` server for MCP and the supplied
`pixen-cli` executable for validation/export. Build and keep all three binaries together.
