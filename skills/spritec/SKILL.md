---
name: spritec
description: Author exact pixel-art JSON and compile it into editable Pixelorama PXO 7 projects and PNG previews using the standalone spritec CLI. Use for static sprites, layered artwork, or explicit frame animations; combine with separate art-direction skills when needed.
---

# spritec

Use `spritec` to turn explicit pixel decisions into editable Pixelorama files.
It targets Pixelorama v1.2.3-stable's RGBA raster subset. Read
[the format reference](references/authoring-format.md) before authoring JSON.
Use [static](examples/static.json) and [animated](examples/animated.json) examples
as structural starting points; derive the actual artwork from the user's brief
and any chosen sprite-design skill.

1. Run `spritec --version`. These instructions describe version 0.1.0 and
   authoring schema 1. If unavailable, use the repository's locally built
   `target/release/spritec` (`spritec.exe` on Windows), or report the missing CLI.
   Building locally with `cargo build --release --locked` is sufficient; do not
   install globally or publish a release without the user's request.
2. Author numeric palette-index grids with exact `#RRGGBBAA` colours. Name cels
   and layers, supply every timeline entry, and choose edit links deliberately.
   A reused grid stays independent unless entries name the same `link` group.
3. Run `spritec validate sprite.json`, then
   `spritec build sprite.json --output sprite.pxo --preview sprite.png`.
   Choose fresh output paths or use `--force` only for outputs being replaced
   within the user's authorized workflow.
4. Open the rendered PNG with the available image-viewing tool. Check silhouettes,
   transparency, layer order and exact placement. Edit the JSON and rebuild until
   it matches the intended art. PNG is an inspection output, not the authoring
   source. Do not replace pixel grids with image generation or flattened imports.
5. For animations, inspect the PXO and render a contact sheet:
   `spritec render sprite.pxo --contact-sheet --columns 8 --output sheet.png`.
   Frame numbers are one-based; `build` previews frame 1. Check durations/tags in
   `spritec inspect sprite.pxo` and inspect individual frames when useful.

Use `spritec unpack existing.pxo --output editable` to produce `sprite.json`
from a supported project. Unsupported features cause conversion errors; do not
work around them by flattening or silently removing them. `inspect` lists those
features without validating all raster data.

Report the source, PXO and preview paths, what was visually checked, and any
compatibility limits. Rust owns validation and archive safety; the skill adds
the authoring/inspection workflow, without prescribing an art style.
