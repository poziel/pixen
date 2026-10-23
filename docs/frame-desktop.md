# Frame desktop

Pixloom's GUI is a Vue application inside one Frame shell, hosted by Tauri.
Frame owns layout, commands, views, dialogs and status. Pixloom supplies the
canvas, Layers and Timeline primary views, and the AI placeholder. There are no
document tabs, secondary sidebar, navigation bar, pane splits or bottom panels.

## Ownership

- `src/desktop.rs`: toolkit-independent application state, document lifetime,
  external refresh, view conversion, render requests and local review.
- `src/session.rs` and `src/mcp_desktop.rs`: existing revisioned edits, safe saves,
  recovery, undo/redo and MCP/agent document coordination.
- `src/viewer_protocol.rs`: managed process lifecycle and shared wire types.
- `src/agent.rs`: preserved AI backend with optional `Wake` callback; attachments
  contain ordinary image data, without window or GPU handles.
- `src-tauri/src/main.rs`: thin native command adapter, file dialogs, managed
  stdin/focus/EOF handling. Blocking work runs away from the UI event loop.
- `ui/editor.ts`: transient view state, event following, bounded render cache and
  playback scheduling. Persistent mutations always go through Rust.
- `ui/CanvasView.vue`: HTML canvas, nearest-neighbor scaling, pan/zoom,
  backgrounds, references and inspection, inside Frame's workspace.

`pixloom-cli` and `pixloom-mcp` do not link Tauri or egui. The desktop depends on
the same Rust library, with a separate lockfile for native dependencies. The
shared target directory is configured in `.cargo/config.toml`.

The managed protocol remains line-delimited stdin messages with JSON replies on
stdout. Startup is acknowledged after Vue mounts; `activate` restores/focuses the
window and acknowledges the request. Activity, inspection and review feed the
same application state. EOF exits the managed desktop. Recovery-file changes are
picked up by snapshot polling. Startup allows 15 seconds for a cold WebView launch.

## Build and checks

Build Frame Core in its own checkout first. Pixloom consumes its public package;
it does not modify Frame source, import its docs app, or register global Bun links.

```powershell
bun install --frozen-lockfile
bun run dev                 # Browser shell; file operations require Tauri
bun run tauri dev
bun run tauri build --debug --no-bundle
cargo build --locked       # Headless tools beside target/debug/pixloom.exe

bun run check
bun test ui
bun run build
cargo fmt --check
cargo test --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

The opt-in native MCP test opens Tauri, publishes edits, activity, inspection and
review, checks reuse/focus acknowledgment, and finishes the project/preview:

```powershell
cargo test --locked --test mcp_protocol tauri_managed_preview_acknowledges_updates_focus_and_shutdown -- --ignored --nocapture
```

For rendered UI verification, launch the test desktop with
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223`, attach
`playwright-cli -s=pixloom attach --cdp http://127.0.0.1:9223`, take a snapshot,
then run `playwright-cli -s=pixloom run-code --filename=scripts/verify-desktop-ui.cjs`.
It exercises a six-frame/two-layer project, native rendering, preview-only solo,
review and the AI placeholder, saving a screenshot under `target/`.
Use the debugging argument only for the test process.

## Behavior boundaries

Layer visibility/solo, preview speed, backgrounds, color modes, inspection and
sprite-sheet conversion are presentation only. Saves and resize use the existing
Rust document path. Clean documents refresh from disk; conflicting external
changes cannot overwrite unsaved edits. Save As can preserve a separate copy.
Imported version-2 cels retain explicit encoding when resized.

AI chat is the accepted frontend omission. Existing live subscription tests stay
opt-in. The backend tests cover its protocol, attachments, preferences and
document behavior without an egui context.

Frame currently supplies its own shell mark; the native application uses Pixloom's
icon. Pixloom view/empty-state SVGs are bundled by Vite. The old egui icon hot-reload
registry is not a runtime Vue feature; SVG changes need the dev server or rebuild.
Legacy egui geometry tokens are replaced by Frame's layout contracts. See
`viewer-themes.md` for the supported theme adapter.
