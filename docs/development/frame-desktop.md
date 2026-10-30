# Frame desktop

Pixen's GUI is a Vue application inside one Frame shell, hosted by Tauri. Frame owns layout, commands, views, dialogs and status. Pixen supplies the canvas, Layers and Timeline primary views, and AI chat. Documents open in workspace tabs in one window. Secondary sidebar, navigation bar, pane splits are disabled. A bottom panel hosts the optional timeline matrix. The empty workspace uses Frame's logo and registered Open/New command actions.

Unavailable Frame regions are configured through `createFrame({ features: ... })`. Frame blocks their commands, shortcuts, and presentation APIs; Pixen does not watch layout changes and revert them. Application commands use `frame.commands.register()` with the `pixen.editor` feature and default global quick-input category. Frame's system commands remain in the `>` category.

## Ownership

- `core/desktop/desktop.rs`: toolkit-independent application state and `DocumentWorkspace`, independent document lifetimes, external refresh, view conversion, render requests and local review.
- `core/automation/session.rs` and `core/automation/mcp_desktop.rs`: existing revisioned edits, safe saves, recovery, undo/redo and MCP/agent document coordination.
- `core/desktop/viewer_protocol.rs`: managed process lifecycle and shared wire types.
- `core/automation/agent.rs`: preserved AI backend with optional `Wake` callback; attachments contain ordinary image data, without window or GPU handles.
- `src-tauri/src/main.rs`: thin native command adapter, file dialogs, managed stdin/focus/EOF handling. Blocking work runs away from the UI event loop.
- `src/editor/editor.ts`: per-document transient view state, serialized document operations, event following, bounded render cache and playback scheduling. Persistent mutations always go through Rust.
- `src/editor/canvas/CanvasView.vue`: HTML canvas, nearest-neighbor scaling, pan/zoom, backgrounds, references and inspection, inside Frame's workspace.

`pixen-cli` and `pixen-mcp` do not link Tauri or egui. The desktop depends on the same Rust library, with a separate lockfile for native dependencies. The shared target directory is configured in `.cargo/config.toml`.

The managed protocol remains line-delimited stdin messages with JSON replies on stdout. Startup is acknowledged after Vue mounts; `activate` restores/focuses the window and acknowledges the request. Activity, inspection and review feed the same application state. EOF exits the managed desktop. Recovery-file changes are picked up by snapshot polling. Startup allows 15 seconds for a cold WebView launch.

## Build and checks

The primary-sidebar toggle uses Frame's built-in toolbar placement API. It appears first in the left status area, before document resolution; Frame owns its icon, command, shortcut and expanded state. After the first successful startup snapshot, the sidebar closes if no artwork is loaded. Launching with artwork keeps Layers open. Closing the final document closes the sidebar again, regardless of whether the document was closed through Ctrl+W, a tab control, or the File menu. Switching documents and polling otherwise preserve the user's sidebar choice.

The follow-up uses `frame.commands.hook('frame.workspace.tab.close', ...)` for command-driven closes and Frame's workspace change event for direct tab controls. Both inspect the resulting Frame tab state; there is no document-state watcher or replacement command handler.

Build Frame Core in its own checkout first. Pixen consumes its public package; it does not modify Frame source, import its docs app, or register global Bun links.

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
bun test tests/frontend
bun run build
cargo fmt --check
cargo test --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

The opt-in native MCP test opens Tauri, publishes edits, activity, inspection and review, checks reuse/focus acknowledgment, and finishes the project/preview:

```powershell
cargo test --locked --test mcp_protocol tauri_managed_preview_acknowledges_updates_focus_and_shutdown -- --ignored --nocapture
```


## Behavior boundaries

Layer visibility/solo, preview speed, backgrounds, color modes, inspection and sprite-sheet conversion are presentation only. Saves and resize use the existing Rust document path. Clean documents refresh from disk; conflicting external changes cannot overwrite unsaved edits. Save As can preserve a separate copy. Imported version-2 cels retain explicit encoding when resized.

AI chat is implemented through Frame chat components and the reusable Frame Server Tauri adapter. Each document owns its conversation. See [AI art chat](../ai/README.md) for setup and behavior.

Frame's shell mark and native application use Pixen's assets/pixen icon. Pixen view/empty-state SVGs are bundled by Vite. The old egui icon hot-reload registry is not a runtime Vue feature; SVG changes need the dev server or rebuild. Legacy egui geometry tokens are replaced by Frame's layout contracts. See `viewer-themes.md` for the supported theme adapter.

## Development host

`bun run dev` launches the Tauri desktop. Tauri starts Vite through `bun run dev:frontend` and loads it inside the native WebView with frontend hot reload. Rust commands, file dialogs, saves and drag-and-drop use Tauri directly. `bun run build` builds the native release application. Tauri calls the internal `build:frontend` script to type-check and bundle Vue; keeping this separate avoids recursively invoking the desktop build. `dev:frontend` is also an internal Tauri hook, rather than the application launch command. There is no browser document backend or web-bridge executable. Opening the Vite URL in a regular browser does not provide a working Pixen application.

Frame owns picker/dialog presentation and overlay styling. Pixen owns document decoding, editing, rendering and saving through the shared Application service. Dimensions, frame position and zoom controls appear in Frame's status bar. Layers reset is a Frame header action, hidden for an empty document.

## Explicit Frame host integration

Core is a Vue/TypeScript library without native dependencies. Pixen supplies `createTauriHost({ files: false })` from `@frame/tauri` when running inside Tauri; browser previews use browser defaults. File dialogs remain owned by Pixen and its existing backend. Native window commands and terminal-independent external opening use the optional adapter.

## Document tabs

Opening artwork or dropping multiple paths adds document tabs; reopening an already open path activates its existing tab. Creating a canvas adds an unsaved tab. Switching retains each session's artwork, save target and preview state, including zoom and pan. Untouched new canvases are clean. Real edits mark their tabs dirty, and saving resets that state. Closing dirty tabs uses Frame's in-app confirmation; closing all tabs restores Frame's welcome page. Managed MCP previews stay bound to their one source document and cannot create, switch or close document sessions.

Native session regressions: `cargo test --locked --test document_workspace --test desktop_service`. Frontend behavior is covered by `bun test tests/frontend`.


## Export artwork

Use **File > Export** (`Ctrl+Shift+E`). See the [export guide](../export/README.md) for supported formats, batch behavior, transparency, and CLI native-project exports.
