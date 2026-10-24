# Code layout

`src/main.ts`, `src/App.vue`, and `src/style.css` are the frontend entry point, application shell, and global stylesheet. Other frontend code is grouped by use:

- `assets/themes/pixloom.yaml`: the application theme, parsed directly by Frame.
- `src/services`: Tauri backend transport and shared response types.
- `src/editor`: editor state and dialogs; canvas, layers, timeline, and shared preview components have their own folders.
- `src/views`: AI and review views.

`core/lib.rs` declares the public Rust modules. Path attributes preserve the existing API while grouping implementations:

- `core/document`: document model, validation, edits, inspection, and optimization.
- `core/formats`: file formats, archive containers, and pixel encodings.
- `core/io`: input/output and image import.
- `core/rendering`: rasterization, previews, video, and exports.
- `core/review`: measurement, comparisons, checkpoints, and review rules.
- `core/desktop`: desktop service, viewer protocol, sheets, and themes.
- `core/automation`: agent processes, MCP, and sessions.
- `core/bin`: CLI and MCP executable entry points.

`tests` mirrors these responsibilities with authoring, formats, rendering, review, automation, desktop, CLI, and frontend suites. Shared sample files remain in `tests/fixtures`. Rust integration targets are registered explicitly in `Cargo.toml`, preserving their names and `cargo test --test <name>` commands. Frontend tests run with `bun test tests/frontend`.
