# Pixen documentation

Pixen (formerly Pixloom) is a desktop pixel-art editor with animation, AI art chat, a command-line interface, and an MCP server.

## Using the application

- [Editing artwork](editing/manual-editing.md): drawing, selection, colors, and recovery.
- [Tool options](editing/tool-options.md): controls and shortcuts for each tool.
- [Animation timeline](editing/timeline.md): frames, cels, timing, and clipboard operations.
- [Exporting artwork](export/README.md): images, animations, video, Aseprite, and Pixelorama.
- [Importing raster artwork](formats/raster-import.md): supported images and animation import.
- [Portable installation and settings](portable.md): storage modes, migration, and updates.
- [AI art chat](ai/README.md): provider setup, artwork context, and references.

## Project files and automation

- [Pixen project files](formats/pixen-project.md): what `.pixen` and `.pixen.json` store.
- [JSON optimization](formats/optimizer.md): lossless source size reduction.
- [MCP setup and workflows](mcp/README.md).
- [MCP tool reference](mcp/mcp-reference.md): the currently implemented API.
- [Art-review APIs](mcp/art-review.md): measurements, checkpoints, and validation.

## Development

- [Source layout](development/code-layout.md).
- [Dependencies and local setup](development/dependencies.md).
- [Desktop architecture](development/frame-desktop.md).
- [Builds and releases](development/releases.md).
- [Project scripts](../scripts/README.md): Bun commands, packaging helpers, and script tests.
- [Themes](development/viewer-themes.md).

## Proposed behavior

These documents describe intended behavior, not additional features available today:

- [Target MCP API contract](specs/mcp-compatibility.md). Its machine-readable [operation inventory](mcp-compatibility/operations.json) and [scenarios](mcp-compatibility/scenarios.json) are also test inputs.
- [Future critic workflow](specs/critic-workflow.md).

Keep guides focused on current behavior. Put active proposals in `specs/`; completed task checklists and one-off verification reports belong in issue or commit history. Generate benchmark/profile output under `target/` rather than committing measurements to this folder.

Write prose paragraphs on a single physical line and use editor soft wrapping for readability. Preserve meaningful line breaks in code blocks, tables, lists, and explicit Markdown hard breaks. The repository formatter uses `proseWrap: "never"`.
