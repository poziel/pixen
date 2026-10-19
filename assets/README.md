# Pixloom icons

Approved pixel-art exports, authored and saved through Pixloom MCP.

| Size | Treatment |
| --- | --- |
| 16, 20 px | Basic yellow/coral colors, darker rear spark, tapered rear points |
| 24, 32 px | Colored semitone rim shading |
| 48, 64 px | Semitone rim and upper-left highlight; no lower-right interior shadow |

Each size has a transparent PNG and editable layered `.pixloom` project.

`pixloom.ico` contains all six exact PNG images, without resampling. Use it for Windows executable resources. The project files preserve separate rear spark, foreground, rim, and lighting layers where applicable.

Use the exact PNG size matching its display slot. For enlarged pixel-art display, use integer scaling and nearest-neighbor filtering.

`preview.png` shows native-size and enlarged icons on light/dark backgrounds. `manifest.json` records filenames and PNG SHA-256 hashes. `pixloom-icons.zip` bundles the delivery files.

The Windows build embeds `pixloom.ico` in all three executables. The inspector uses `pixloom-48.png` for its window icon. Regenerate the ICO, manifest, preview, and delivery ZIP with `python assets/package_icons.py` (requires Pillow).
