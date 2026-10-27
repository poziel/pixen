# Timeline matrix: first working sketch

The left status-bar button immediately after the primary-sidebar toggle switches between the layer/timeline sidebar views and a combined bottom matrix. Its icon and expanded state reflect the current layout. Closing the bottom pane restores the sidebar views. The bottom pane uses Frame registration and resizing, with `showHeader: false` and no panel view bar. Maximize/close controls share the matrix's single toolbar. Narrow panes progressively group creation, cell actions, and playback into Frame dropdown menus rather than wrapping the toolbar.

## Interactions

- Click a cell to select its layer and frame. Arrow keys navigate cells.
- Select frame cels selects an entire column; it is available in the toolbar and frame-header context menu.
- Preview speed has the same dropdown in the sidebar and matrix. Use frame timings plays stored per-frame durations; an FPS option previews every frame at a uniform rate without changing the document.
- Onion skin and frame comparison are display-only while drawing. Stroke previews and merged sampling use the original artwork; reference tints/differences are never committed to a cel.
- Double-click a layer or frame header (or press F2) to rename it. Enter saves; Escape cancels.
- Double-click a duration caption to edit milliseconds. Frame-header menus also offer Frame duration and Duration of selected frames. Selected frame IDs are deduplicated across layers and saved as one undoable edit; stale dialogs cannot overwrite newer document changes. Column numbers always reflect timeline order, with custom names retained in header labels/tooltips.
- Drag layer headers vertically or frame headers horizontally to reorder whole rows/columns. Insertion markers show the drop position. Alt+Up/Down on layer headers and Alt+Left/Right on frame headers provide keyboard reordering. Cell selection stays attached to stable IDs.
- Drag selects a rectangle across frames and layers, with edge scrolling. Shift-click or Shift-arrow extends from the selection anchor horizontally, vertically, or diagonally. Ctrl/Cmd toggles individual cells across any layers; Ctrl/Cmd-drag adds a rectangle. Ctrl/Cmd+A selects the matrix.
- Select layer cells selects the entire active layer's timeline.
- Link cells groups selected cells separately within each layer, with confirmation before replacement. The selected cell in the active frame supplies that layer's artwork; if it is not selected, the earliest selected frame supplies it. Rows containing only one selected cell are unchanged. Edits then propagate through each link group, including initially empty cells.
- Unlink selected cels detaches the selection; later drawing uses copy-on-write.
- The cell context menu exposes Unlink this cell and Unlink selected cells. Both preserve pixels; one cell can be detached without changing the remaining link partners. The toolbar calls this Unlink selected cells as well.
- Copy selected cels (Ctrl/Cmd+C) snapshots content. Paste into selected cels (Ctrl/Cmd+V) fills selected slots from a single copied cel, or preserves a copied group's layout at one destination/a matching selection. This works across layers and frames without creating links. See [the interaction review](timeline-interaction-review.md) for clipboard limits and toolbar recommendations.
- Cut selected cels (Ctrl/Cmd+X) snapshots first and clears only the captured source selection after the copy succeeds. Immediate paste waits for the cut to finish. Linked partners outside the cut selection remain intact.
- Delete/Backspace or Clear selected cells clears exactly the selected artwork in one undoable transaction. Selected linked cells detach; unselected link partners retain their pixels. Layers and frames remain in place. A selection containing a locked layer disables cell mutations.
- New layer creates empty cells across the timeline. New frame inserts an independent copy of the current frame across every layer and retains its duration; Empty frame explicitly inserts a blank frame. Duplicate Frame remains available in header context menus. The sidebar's Add/New frame follows the same copy-current default.
- Copy previous frame here fills an existing column from its immediate predecessor without inserting a frame or changing the destination's timing. Use this when continuing an animation in a preallocated blank timeline.
- Cell operations use the existing revision-checked, atomic, undoable edit history. Managed previews, alternate views, and locked layers retain their edit restrictions.
- Context menus offer Delete Frame/Delete Layer with confirmation, plus Copy to the Right/Left into adjacent existing frames. A frame-header copy copies every layer in that column. Cell copies affect selected cells, skip timeline boundaries, and process from the far end to avoid overwriting later sources. Destination copies are independent.
- Selection details and keyboard hints use the application's real status bar. The matrix has no footer/status strip.

## Components and limits

Buttons, icons, tooltips, dropdowns, scrolling, confirmation dialogs, and the pane come from Frame. Frame now exposes a per-view `showHeader` option for bottom and secondary panes. `TimelineMatrix.vue`, `TimelineMatrixToolbar.vue`, and the matrix selection modules remain Pixen-specific compositions. A reusable keyboard-navigable selectable grid or responsive grouped toolbar could eventually move into Frame; animation semantics should stay in Pixen.

Dragging cell artwork itself between slots remains a future refinement. Header reordering, duration editing, and rename/delete menus are implemented. Large timelines currently render all cells rather than virtualizing them.

`scripts/verify-timeline-matrix.cjs` exercises the UI with fixture artwork and saves desktop/compact screenshots under `output/`. Rust editing and desktop-service tests exercise real cell operations, native paint propagation, independent copies, rollback, and undo.
