---
name: new
title: Create a sprite document
description: Create a new Pixen document with the right canvas size, explicit palette and layer structure, so later work does not have to fight the setup. Use when starting fresh rather than editing an existing sprite.
---

# Create a sprite document

Set-up mistakes are expensive to undo: a canvas that is the wrong size means redrawing, and a palette chosen after the art means repainting. Get these four things right and the rest of the work is drawing.

## Procedure

1. **`preflight`.** Stop if not ready. Read new-tool metadata from structuredContent, carry the returned project_id and revision into edits, and refresh sprite_info after a stale revision. Use only operations preflight advertises.

2. **Create the canvas.**

   ```
   sprite_manage op="new" width=32 height=32 palette=["#00000000","#202030FF","#E0D0B0FF"] output_path="…/knight.pixen"
   ```

   - Pixen stores palette indices with RGBA entries. There is no upstream RGB/indexed mode switch; index 0 is not necessarily transparent. Supply an explicit palette. In desktop mode omit output_path: the application owns the temporary destination.
   - Size to the game's grid if there is one: a 16×16 tile game wants character sprites that are a multiple of 16 in at least one dimension.
   - Leave headroom. A 32×32 canvas for a 28px-tall character gives room for a weapon raise or a jump squash without a later canvas resize.

3. **Set the palette before drawing.**

   ```
   sprite_info
   ```

   Inspect the returned exact palette slots. Palette presets and ramps are planned capabilities; check preflight before calling them. Choose the intended colors explicitly at creation. See `rules://01-palette-and-color`.

4. **Build the layer structure** in one batch, so it is one undo step:

   ```
   layer op="batch" expected_revision=<current revision> ops=[
     {op:"create", name:"sketch"},
     {op:"create", name:"base"},
     {op:"create", name:"shading"},
     {op:"create", name:"outline"},
   ]
   ```

   For anything that will be animated, build the full character rig instead — see `pixen:rig`. Splitting baked pixels apart later is real work.

5. **Save immediately**, so the user has a file and undo has an anchor:

   ```
   save_project project_id=<returned project ID>
   ```

   Standalone uses the destination supplied at creation. Desktop Save/Save As belongs to the application; sprite_manage save_as is not implemented yet. Saving is separate from undo history.

6. **Confirm** what you made in one line: size, palette-index model, palette, layers.

## Then

`pixen:draw` to block in the art.

## Related

`rules://01-palette-and-color`, `rules://06-layers-and-rigging`.
