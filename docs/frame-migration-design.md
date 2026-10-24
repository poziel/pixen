# Pixloom: Frame and Tauri migration

## Intent and agreed scope

Replace the egui desktop with a Bun-managed Vue/TypeScript application hosted by Tauri. Frame is the complete application container. Preserve existing Pixloom functionality, file compatibility, CLI and MCP workflows. The AI conversation frontend is the sole accepted feature omission; its Rust backend must remain usable and independent of any UI toolkit.

Before migration, the existing viewer/theme work was committed and pushed to `origin/feature/art-conversation-panel` at `aff2986ae030319ad03db8ce1b56a74e7d8ad145`. Local and remote-tracking commits matched and the working tree was clean after pushing. This is the recovery baseline, not a claim that baseline tests pass.

## Shell contract

- One stable `createFrame()` instance and `FrameShell` own the full desktop container, title bar, menus, commands, layout, status and view lifecycle.
- Layers, Timeline (the current Animation view), and AI chat are separate primary sidebar views. Only one is selected at a time; selecting the active view can collapse the sidebar.
- The AI chat view is a small unavailable-state placeholder, with no conversation frontend or automatic agent startup. This resolves the requested placement while honoring the accepted frontend omission.
- The Vue canvas occupies the single central workspace, with no document tabs or pane splitting.
- No secondary sidebar, navigation bar, bottom/additional panel, or shell controls that offer those regions. Use public Frame configuration and instance APIs; do not maintain competing layout state.
- Dialogs and settings use overlays within the shell, not extra docked panels or workspace tabs.
- Frame is consumed as a library dependency, never by copying or importing its documentation/demo application.

## Approach

Recommended: extract a frontend-independent Rust application service and make Tauri a thin adapter. Vue sends typed requests and subscribes to state/events; Rust remains authoritative for documents, persistence, edits, history, rendering/inspection and AI processes. The CLI/MCP can continue using those same Rust facilities without a webview.

Embedding egui beneath a Frame shell would retain the toolkit coupling and would not satisfy the Vue canvas requirement. Reimplementing the engine in TypeScript would duplicate semantics and risk divergence between desktop and MCP. Neither fits the requested outcome.

## Backend boundary

Existing `Session`, loaders, exporters, rasterizer, edit validation, revision checks, undo/redo, inspection and review modules remain the foundation. Extract desktop coordination currently held in viewer/conversation modules into ordinary Rust types. Move shared activity/review DTOs and process protocol out of UI modules. Replace `egui::Context` wakeups in the agent with a UI-independent notification mechanism.

Tauri commands translate inputs into service calls and return serializable results/errors. They do not implement a separate save or edit path. Long-running native dialogs, file work and agent work must not block the UI event loop. Subscription teardown and shutdown must release watchers, worker processes and temporary resources.

The transport carries revisioned state and explicit document/frame/layer IDs. Late responses must not replace a newer document or revision. Vue owns transient interactions such as pointer pan/zoom and animation scheduling; it uses Rust frame data and durations. Cached render data should avoid rerasterizing or transferring the entire sprite on every animation tick. Rust validates persistent mutations and retains existing safe-output behavior.

Preserve `pixloom` as the GUI executable and `pixloom-cli`/`pixloom-mcp` as headless entry points. Preserve file arguments and the managed viewer lifecycle: open acknowledgment, activate/focus, activity, inspection, review, stdin EOF and close. Review startup timeout/error handling for webview startup rather than silently retaining OpenGL-specific assumptions. GUI availability must not become a prerequisite for headless engine operations.

## Feature parity contract

Implementation must inventory existing viewer functions/tests and maintain an explicit verification matrix. Required areas include:

- Empty launch, file launch, native open, new artwork, canvas resizing and reversible dialog previews.
- Save/Save As through the existing document service, format compatibility, recovery behavior and external file refresh.
- Crisp pixel canvas, fit/reset, zoom/pan, checkerboard/transparent/black/white backgrounds and sprite-sheet/animation switching.
- Timeline thumbnails, selection, playback/pause/stop, stepping, keyboard controls and preview speed independent of saved frame timing. Closing the sidebar does not stop playback.
- Layers ordered as today, thumbnails, selection, preview visibility, solo and reset. Preview visibility does not change saved layer data.
- Onion skin, frame comparison/reference selection, isolated layers, inspection/review overlays and color/review controls.
- MCP-driven document refresh, busy/activity state, explicit target following, inspection/review updates, focus and process lifecycle. Managed-view restrictions must remain enforced.
- Menus, existing applicable shortcuts, settings, status, native title/window controls and icons/themes adapted to Frame's tokens.
- AI backend: process integration, model preferences, events, approvals, attachments, cancellation, checkpoints/rewind and cleanup survive extraction. No replacement chat UI is required now; retain headless tests for backend behavior.

Existing exports and other headless capabilities remain available through their existing CLI/MCP contracts; this migration does not promise brand-new GUI tools that the current viewer lacks.

## Dependencies and local integration

Use Vue 3, TypeScript, Vite, Bun and Tauri 2 with Frame's public packages. Prefer project-local package references to already-built Frame libraries while developing; keep Vue deduplicated. Do not modify Frame or global Bun link registration as part of Pixloom setup. If Frame packages require rebuilding, determine the narrow required build and filesystem permission before writing outside Pixloom. Optional themes may be consumed where useful; optional Features are not required to build a custom pixel canvas.

Keep build artifacts ignored and document the new Bun/native build commands and executable distribution requirements. Exact folder layout, IPC schemas, caching and test tooling are implementation choices. Preserve the repository's existing Rust library and headless entry points.

## Verification and completion

First record baseline Rust test results. Add meaningful backend tests for extracted service behavior and managed protocol/lifecycle; adapt existing tests away from egui-specific assumptions without dropping behavioral assertions. Check Rust formatting, relevant tests and native compilation, plus Vue type checking and a production frontend build.

Exercise the running desktop with representative static, layered and animated projects, save/reopen and external updates. Test MCP managed preview end to end against the new GUI executable, including repeated updates, target selection, inspection/review, focus and shutdown. A browser-only check is insufficient evidence for Tauri IPC or native controls. Do not start paid AI work merely to verify the migration; use existing backend tests/mocks unless separately authorized.

Report any missing parity or unverified native behavior explicitly. The desktop switch is complete only when required parity is implemented and tested; a mounted Frame shell alone is an intermediate result. Remove egui from the final production dependency graph after its backend coupling is eliminated and the new desktop passes verification.

## Approval

This document records the requested layout and recommended backend boundary. Implementation follows approval of this migration contract. Only the pre-migration checkpoint has been committed/pushed; this design is separate from that checkpoint.
