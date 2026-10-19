---
name: pixloom
description: Create, edit, inspect, and export layered pixel art and animations with Pixloom through its connected MCP tools or standalone CLI. Use for Pixloom projects, exact pixel-art JSON, and supported native project conversions; combine with separate pixel-art guidance for artistic decisions.
---

# Pixloom

Use the interface available in the current agent environment. This skill provides
instructions; it does not install executables or register an MCP connection.

## Choose an interface

- **Connected MCP:** prefer it for interactive creation, targeted edits, layers,
  palettes, animation, checkpoints, and visual review. Read
  [the MCP workflow](references/mcp.md). CLI installation is unnecessary for
  operations supported by the connected server.
- **CLI:** use it for explicit JSON authoring, validation, batch conversion,
  packing/optimization, rendering, and video export. Read
  [the CLI workflow](references/cli.md). It requires terminal access to
  `pixloom-cli`; an MCP connection is unnecessary.
- If both are available, choose by task. Keep active MCP edits in that session;
  use CLI on saved files or separate exports. Do not externally overwrite an
  active project's destination.
- If the required interface is unavailable, explain what is missing. A path in
  this skill does not connect a server. Do not assume Cargo, modify global
  installation, or change client configuration without the user's request.

MCP clients launch `pixloom-mcp` (`pixloom-mcp.exe` on Windows); `pixloom-cli`
provides terminal commands and `pixloom` is the desktop inspector. Use configured
paths or discover available executables rather than hardcoding a developer path.
The client owns MCP startup; do not launch a second server for its connection.

## Shared working principles

Preserve the brief, intended display size, existing style, editable layers, and
intentional animation timing. Use a separate pixel-art skill when available for
artistic decisions; this skill supplies Pixloom operations and evidence.

Prefer `.pixloom` for saved editable projects. `.pixloom.json` is the explicit
text-authoring format; `.pixloom` is a ZIP container, not renamed JSON. Open other
supported formats when needed and export native formats explicitly when requested.
Preserve sources and use fresh destinations unless replacement is authorized.

Inspect actual image results after meaningful changes. Tool success and valid
files do not establish readability or motion quality. Compare against the brief
and baseline, and report checks actually performed. Save the editable project
alongside requested exports; distinguish session-local review checkpoints from
files that persist after the server exits.

Read [the authoring format](references/authoring-format.md) only when writing or
interpreting JSON. MCP drawing does not require hand-authoring that document.
