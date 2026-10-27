# Timeline interaction review

## Terms and clipboard

Use **cel** for animation artwork at a layer/frame intersection. **Cell** remains a generic grid/code term; backend operation IDs are retained for compatibility. User-facing timeline actions now say cel/cels.

Ctrl/Cmd+C and **Copy selected cels** capture pixels, transparency, and per-cel opacity. Ctrl/Cmd+V and **Paste into selected cels** replace full destination cels in one undoable transaction, detach destination links, and import colors into the target palette. Source layer opacity remains a layer property rather than being baked into copied pixels.

One copied cel fills all selected destinations, including different layers/frames. A group preserves its sparse layout: select one starting cel, or select a matching destination layout. Overflow, size mismatches, locked destinations, or stale copies are rejected without partial edits. The clipboard is internal to Pixen, survives source editing/deletion and document switches, and supports equal-sized artworks; it does not yet import/export the system image/text clipboard. Renaming fields retain their normal text copy/paste behavior.

The previous **Copy to selected** action inferred a source separately in each row. It has been replaced by explicit copy and paste actions, available in the toolbar and cel context menu, with shortcut hints.

## Current surfaces

| Surface | Current responsibilities |
| --- | --- |
| Matrix toolbar | Transport controls; new layer, copied new frame, explicit empty frame; select layer/frame cels; link/unlink; cut/copy/paste; clear; separate preview timing control; maximize/close |
| Cel context menu | Copy/paste; link selection; unlink one or selected cels; adjacent directional copy; clear; delete frame/layer |
| Header context menus | Rename, duplicate, delete; single/range duration editing; copy previous frame into an existing column; frame-column directional copy |
| Sidebar tabs | Structure lists with creation, rename, duplication, deletion and group reordering; layer visibility/solo; timeline playback and preview-speed menu |
| App shell | Undo/redo through navigation; view/inspection modes including onion skin and frame comparison; shared status information |

## Recommended next steps

1. **Move cels by dragging:** distinguish moving artwork between slots from moving whole layer/frame headers. Clipboard cut/paste already provides explicit moves.

## Implemented follow-up

Select frame cels is now in the toolbar and column-header context menu. Cut selected cels (Ctrl/Cmd+X) is available in the toolbar, cel context menu, and keyboard handling; failed copying never clears the source. Preview-speed menus share one definition between the sidebar and matrix, including the existing FPS choices and Use frame timings.

Frame duration is the hold time before advancing: 50 ms is 1/20 second, 55 ms is 0.055 second, and 100 ms is 1/10 second. Saved animation/video exports use these durations. An FPS preview override intentionally ignores them; Use frame timings previews them directly. New empty frames inherit the active frame's duration, falling back to 100 ms, and copied new frames retain their source timing. Double-clicking a duration caption now opens the millisecond editor; frame-header context menus can change one or several selected frames atomically.

## Sequential animation workflow

New Frame now copies the active frame into the next position, independently across all layers. Empty Frame remains explicit. To fill step 4 in an already-created twelve-frame sequence, right-click frame 4 and choose Copy previous frame here, then draw the next pose. This preserves frame 4's timing and does not insert another frame or overwrite later poses. There is no implicit linking or automatic filling of the remaining blank frames.

This borrows the useful split in [Aseprite's New Frame documentation](https://www.aseprite.org/docs/new-frame/): copied new frames versus explicitly empty ones. [Pixelorama's timeline manual](https://pixelorama.org/user_manual/user_interface/timeline/) also distinguishes adding/cloning and exposes duration through frame properties. [Aseprite's duration documentation](https://www.aseprite.org/docs/frame-duration/) supports changing a selected range together. Pixen keeps these interactions limited to the existing matrix rather than adding broader animation machinery.

The redundant Timeline heading is removed. The six transport controls remain one group; preview speed is separately placed at the right, displaying the current timing mode when space permits. The Cut control uses a locally bundled Lucide scissors component because `scissors` was not available in the Codicon set.

Drawing in onion-skin and frame-comparison modes now uses clean original cel pixels. Inspection tints are applied to live previews only, matching native rendering rules, and merged sampling excludes inspection overlays. Managed documents, converted/sheet views, non-color representations, and resize previews retain their existing restrictions.

Keep frequent playback and copy/paste operations in the top toolbar. Keep directional copy, rename, duplicate-layer and destructive structural actions primarily in context menus. Do not duplicate existing shell undo/redo or put every structural command into the compact toolbar. Sidebar-tab copy semantics should remain explicit: whole-layer/frame duplication is different from copying cel contents.
