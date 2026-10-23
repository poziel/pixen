# Viewer themes and panels

Pixloom uses one left activity rail and one selected sidebar: Layers, Animation, or AI conversation. Clicking the active activity collapses it. Playback controls belong to Animation; hiding the panel does not stop playback or AI processing. The rail and active sidebar share one rounded bordered container, including when collapsed to the rail alone. Drag its right edge to resize; the edge highlights with the accent color. All panels use the same width limits and reserve a minimum canvas width. The canvas is a separate rounded darker surface. Top and status bars share `bar_height`.

## Editing the theme

`assets/themes/manifest.json` lists theme IDs, their JSON files, and the default ID. Duplicate `2026-dark.json`, register the new file, and change `default`. Alternatively set `PIXLOOM_THEME` to a registered ID or a JSON file path before starting Pixloom. Restart the application after editing theme configuration. Missing fields inherit the bundled 2026 Dark defaults; invalid JSON falls back to those defaults.

The `colors` object defines semantic RGB hex colors. The `metrics` object defines logical pixel sizes (scaled with display DPI): bar/header heights, activity rail/sidebar widths, font sizes, borders, corner radii, spacing, padding, icon/button sizes, empty-workspace icon size, and menu width/row height/padding/separator thickness. These are native egui theme tokens, not CSS. Content-specific drawing geometry, artwork pixels, and native window hit regions are not CSS properties and remain code-owned.

The menu has its own raised surface, full-width separators, and transparent menu rows. General buttons have a filled background. Icon controls are transparent until hovered, with a brighter glyph and hover surface.

`font_file` selects a TTF/OTF file. By default Segoe UI is loaded on Windows when available; other systems use the bundled proportional font.

## Icons

`assets/icons/registry.json` maps semantic names to editable SVG asset basenames. Themes supply tint colors; SVGs supply shapes. Use white SVG artwork so theme tinting works. `empty_workspace` selects the large empty-canvas mark, independently tinted by `empty_icon`. SVG edits refresh while running; registry mappings are read at rendering time.

The application background, including the gutters between containers, shares the top/status bar shell color. The application mark is the approved Pixloom 20px PNG beside the menu button.

The source checkout provides runtime assets. Theme defaults are embedded as a fallback. A packaged distribution should ship the themes and icons directories beside its executable.
