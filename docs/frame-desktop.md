# Frame desktop

Pixen's GUI is a Vue application inside one Frame shell, hosted by Tauri.
Frame owns layout, commands, views, dialogs and status. Pixen supplies the
canvas, Layers and Timeline primary views, and the AI placeholder. Documents open
in workspace tabs in one window. Secondary sidebar, navigation bar, pane splits
and bottom panels are disabled. The empty workspace uses Frame's logo and
registered Open/New command actions.

Unavailable Frame regions are configured through `createFrame({ features: ... })`.
Frame blocks their commands, shortcuts, and presentation APIs; Pixen does not
watch layout changes and revert them. Application commands use
`frame.commands.register()` with the `pixen.editor` feature and default global
quick-input category. Frame's system commands remain in the `>` category.

## Ownership

- `core/desktop.rs`: toolkit-independent application state and `DocumentWorkspace`,
  independent document lifetimes, external refresh, view conversion, render requests
  and local review.
- `core/session.rs` and `core/mcp_desktop.rs`: existing revisioned edits, safe saves,
  recovery, undo/redo and MCP/agent document coordination.
- `core/viewer_protocol.rs`: managed process lifecycle and shared wire types.
- `core/agent.rs`: preserved AI backend with optional `Wake` callback; attachments
  contain ordinary image data, without window or GPU handles.
- `src-tauri/src/main.rs`: thin native command adapter, file dialogs, managed
  stdin/focus/EOF handling. Blocking work runs away from the UI event loop.
- `src/editor.ts`: per-document transient view state, serialized document operations,
  event following, bounded render cache and playback scheduling. Persistent mutations
  always go through Rust.
- `src/CanvasView.vue`: HTML canvas, nearest-neighbor scaling, pan/zoom,
  backgrounds, references and inspection, inside Frame's workspace.

`pixen-cli` and `pixen-mcp` do not link Tauri or egui. The desktop depends on
the same Rust library, with a separate lockfile for native dependencies. The
shared target directory is configured in `.cargo/config.toml`.

The managed protocol remains line-delimited stdin messages with JSON replies on
stdout. Startup is acknowledged after Vue mounts; `activate` restores/focuses the
window and acknowledges the request. Activity, inspection and review feed the
same application state. EOF exits the managed desktop. Recovery-file changes are
picked up by snapshot polling. Startup allows 15 seconds for a cold WebView launch.

## Build and checks

The primary-sidebar toggle uses Frame's built-in toolbar placement API. It appears
first in the left status area, before document resolution; Frame owns its icon,
command, shortcut and expanded state. After the first successful startup snapshot,
the sidebar closes if no artwork is loaded. Launching with artwork keeps Layers
open. Closing the final document closes the sidebar again, regardless of whether
the document was closed through Ctrl+W, a tab control, or the File menu. Switching
documents and polling otherwise preserve the user's sidebar choice.

The follow-up uses `frame.commands.hook('frame.workspace.tab.close', ...)`
for command-driven closes and Frame's workspace change event for direct tab
controls. Both inspect the resulting Frame tab state; there is no document-state
watcher or replacement command handler.

Build Frame Core in its own checkout first. Pixen consumes its public package;
it does not modify Frame source, import its docs app, or register global Bun links.

```powershell
bun install --frozen-lockfile
bun run dev                 # Tauri desktop with frontend hot reload
bun run tauri dev
bun run build:debug         # Debug Tauri desktop
bun run build               # Release Tauri desktop, no installer
bun run build:cli           # Release CLI only, without Tauri
bun run build:mcp           # Release MCP only, without Tauri
bun run build:tools         # Both headless tools, without Tauri

bun run check
bun test src
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
`playwright-cli -s=pixen attach --cdp http://127.0.0.1:9223`, take a snapshot,
then run `playwright-cli -s=pixen run-code --filename=scripts/verify-desktop-ui.cjs`.
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

Frame's shell mark and native application use Pixen's assets/pixen icon. Pixen view/empty-state SVGs are bundled by Vite. The old egui icon hot-reload
registry is not a runtime Vue feature; SVG changes need the dev server or rebuild.
Legacy egui geometry tokens are replaced by Frame's layout contracts. See
`viewer-themes.md` for the supported theme adapter.

## Development host

`bun run dev` launches the Tauri desktop. Tauri starts Vite through
`bun run dev:frontend` and loads it inside the native WebView with frontend hot
reload. Rust commands, file dialogs, saves and drag-and-drop use Tauri directly.
`bun run build` builds the native release application. Tauri calls the internal
`build:frontend` script to type-check and bundle Vue; keeping this separate avoids
recursively invoking the desktop build. `dev:frontend` is also an internal Tauri
hook, rather than the application launch command.
There is no browser document backend or web-bridge executable. Opening the Vite
URL in a regular browser does not provide a working Pixen application.

Frame owns picker/dialog presentation and overlay styling. Pixen owns document
decoding, editing, rendering and saving through the shared Application service.
Dimensions, frame position and zoom controls appear in Frame's status bar.
Layers reset is a Frame header action, hidden for an empty document.

## Explicit Frame host integration

Core is a Vue/TypeScript library without native dependencies. Pixen supplies `createTauriHost({ files: false })` from `@frame/tauri` when running inside Tauri; browser previews use browser defaults. File dialogs remain owned by Pixen and its existing backend. Native window commands and terminal-independent external opening use the optional adapter.

## Document tabs

Opening artwork or dropping multiple paths adds document tabs; reopening an already open path activates its existing tab. Creating a canvas adds an unsaved tab. Switching retains each session's artwork, save target and preview state, including zoom and pan. Untouched new canvases are clean. Real edits mark their tabs dirty, and saving resets that state. Closing dirty tabs uses Frame's in-app confirmation; closing all tabs restores Frame's welcome page. Managed MCP previews stay bound to their one source document and cannot create, switch or close document sessions.

Native session regressions: `cargo test --locked --test document_workspace --test desktop_service`. Frontend integration with mocked native adapters: start Vite on port 1458 (or set `PIXEN_TEST_URL`) and run `node scripts/verify-document-tabs.cjs` with Playwright available. That check covers welcome actions, progressive menu overflow, centered title, fixed control sizes, multi-file drop routing, tab switching, clean closing and reopening after the final tab closes.

Tab appearance and playback regression: `node scripts/verify-tab-states.cjs` against Vite on port 1459 (or `PIXEN_TEST_URL`). It checks theme inheritance, hidden single-tab strips, dirty indicators and save/reset, Frame close dialogs, and retention of decoded layer thumbnails during delayed animation renders.

## Export artwork

Use **File → Export…** (`Ctrl+Shift+E`) to export the open artwork without changing its project path or dirty state.

Choose one shared file name and destination folder, then add export entries. Each entry starts with its export kind: current frame, sprite sheet, animation, video or editable project. Frame Select disables incompatible formats; changing kind resets an incompatible selection. Frame/sheet support PNG, JPEG, GIF, WebP, SVG, BMP, TIFF and APNG; animation supports GIF/WebP/APNG; video supports MP4/WebM and requires FFmpeg; editable project uses **Pixen JSON (.pixen.json)**. Each entry has independent parameters and can be removed. Export and Cancel use the Frame modal footer.

Distinct extensions share a stem (hero.png, hero.jpeg). Repeated extensions include the export kind (hero-frame.png, hero-sheet.png), with a numeric suffix for repeated kinds. The dialog previews the exact names. One replacement confirmation covers existing files, and all outputs are encoded into temporary files before any destination is replaced. An encoding failure leaves existing exports unchanged.

Sprite sheets arrange frames left to right in timeline order. Set columns or rows; unused cells remain transparent. Nearest-neighbor scale is an integer from 1–64, with a maximum output canvas of 4096 pixels per side and 4,194,304 pixels. JPEG/BMP/video require an opaque background; GIF with partial transparency requires a background too. JSON preserves all editable source data. JSON destinations use `.pixen.json`; the binary project extension remains `.pixloom`.