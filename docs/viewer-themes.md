# Desktop themes and views

Layers, Timeline and the AI placeholder are separate primary sidebar views in
Frame. Hiding Timeline does not stop playback. The canvas is a central Vue
workspace with its own background, pan and zoom.

`assets/themes/pixloom.yaml` is the default **Pixloom** theme. It uses Frame's
native token format directly: there is no TypeScript color mapper or separate
Pixloom color registry. The accent is the logo's coral-orange `#FE725E`, with
warm dark surfaces and subdued orange list selections. Fonts remain Segoe UI
at 13px with a system sans-serif fallback.

`assets/themes/manifest.json` selects the default theme ID and maps IDs to files.
Set `PIXLOOM_THEME` to a registered ID or a theme file path before launch.
Frame accepts JSON, JSONC, JSON5, YAML (`.yaml`/`.yml`), TOML and INI. Restart
after changing selection. `core/desktop/theme.rs` only reads bounded file text
and reports its format; Frame parses and validates it. Missing, unreadable,
oversized or unsupported files use the embedded Pixloom YAML. If a selected
file fails Frame validation, the UI keeps the bundled theme and logs a warning.

Colors and typography belong in `tokens`, using nested or dotted paths:

```yaml
tokens:
  frame:
    accent: "#FE725E"
    canvas: "#151313"
    font.size.body: "13px"
```

Application views and dialogs inherit the same Frame tokens as shell chrome
and overlays. Deliberate checkerboard/black/white canvas previews remain neutral
so artwork colors can be judged independently. Sidebar width is application
layout configuration in `src/App.vue`, rather than a theme metric.

The former `2026-dark.json` format (`colors`, `metrics`, `font_file`) is replaced
by the Frame token document. Font-family tokens select installed fonts; they do
not load arbitrary font files. Custom font assets need explicit host loading.

Distributions can ship `themes/` beside the executable. The source checkout's
`assets/themes/` is preferred when available; a default is embedded. The frontend
and SVG icons are bundled into the desktop by Tauri/Vite. SVG changes require a
frontend rebuild or development server; the old icon registry remains an asset
reference rather than a runtime Vue feature.

`assets/fonts/Hack-Regular.ttf` and its license keep contact-sheet and review
labels deterministic without loading egui's default font definitions.
