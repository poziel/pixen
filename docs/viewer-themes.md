# Desktop themes and views

Layers, Timeline and the AI placeholder are separate primary sidebar views in
Frame. Hiding Timeline does not stop playback. The canvas is a central Vue
workspace with its own background, pan and zoom.

`assets/themes/manifest.json` lists the default theme. `PIXLOOM_THEME` selects a
registered ID or JSON file before launch. Missing or invalid fields fall back
to the bundled 2026 Dark theme. Restart after editing native theme selection.

`src/theme.rs` reads the registry without a UI dependency. `ui/theme.ts` maps
shell/panel/canvas/text/muted/accent/border/selection/hover/button/raised colors
to Frame tokens and Pixloom content variables. Font size, initial sidebar width,
and a selected TTF/OTF font are carried across. Frame owns the remaining shell
geometry, spacing, menus and control dimensions; old egui-only metric names no
longer configure those elements.

Distributions can ship `themes/` beside the executable. The source checkout's
`assets/themes/` is preferred when available; a default is embedded. The frontend
and SVG icons are bundled into the desktop by Tauri/Vite. SVG changes require a
frontend rebuild or development server; the old icon registry remains an asset
reference rather than a runtime Vue feature.

`assets/fonts/Hack-Regular.ttf` and its license keep contact-sheet and review
labels deterministic without loading egui's default font definitions.
