# Manual pixel editing

The **Layers** and **Timeline** view headers provide add and delete buttons.
Add creates a transparent item with a default name. Delete asks for confirmation;
the last layer or frame cannot be deleted. A new frame inherits the selected
frame's duration.

Right-click a row to create an item immediately below it, rename, duplicate, or
delete it. Double-click the name or press F2 on a focused row to rename it in
place. Enter, Tab, and clicking away save; Escape cancels. Switching views also
commits an active rename. Duplicates appear
directly below their source with a “copy” suffix and independent artwork.
Right-click empty list space to create an item.

Drag rows to reorder them. Ctrl/Command-click toggles additional rows; Shift-click
selects a range, so multiple rows can move as a group. Alt+Up/Down reorders the
focused row or selected group using the keyboard. Each drop is one undo step.
A floating thumbnail/name preview follows the pointer during dragging; groups
show their count and up to three item previews. The insertion bar marks the drop
position.

All four operations use the document's undo/redo history. Reordering preserves
the selected item's identity and its artwork. Custom frame names are stored in
native project metadata (`pixen.frame_name`). Managed previews and alternate
views remain read-only.

Open the **Tools** tab in the primary sidebar. Choose a tool independently for
each mouse button using the Left button and Right button selectors. Clicking a
tool icon assigns it to the left button; right-clicking assigns it to the right.
The initial pair is pencil and eraser.
Blue highlights mark the left assignment and orange highlights mark the right.
Each occupies its half of the tool button, so assigning both buttons to the same
tool shows both colors. Pixel-aligned cursor outlines use the same colors and
show both tool footprints. Each button retains independent pencil and eraser settings.

Draw on the selected visible layer of the selected frame. Painting pauses
animation. Manual editing uses the original color artwork; switch out of sheet,
comparison, silhouette, or resized preview modes first. Managed AI previews are
read-only. Completed gestures enter the same recoverable document history as AI
edits and are included when saving the document.

- Rectangle and ellipse selections use a drag. Freehand selection follows the
  drag path and closes it on release. Polygon selection uses clicks for vertices;
  double-click the last vertex with its assigned button to finish.
- Color selection and bucket fill can use contiguous pixels or all matching
  pixels, with an adjustable RGBA tolerance. Selections support replace, add,
  and subtract, and constrain drawing, erasing, filling, text, and moving.
- Move translates the selection, or the entire active cel when no selection
  exists. Enable Copy when moving to retain the source. Pixels outside the
  canvas are clipped.
- Pencil and eraser each have independent size, density, and opacity controls.
  Opacity is applied once per pixel per gesture, including overlapping stamps.
- Colored paper fills the active cel inside the selection, or the entire cel.
  Text is rasterized to pixel edges using the configured text and size; it is
  committed as pixels and can be undone as one operation.
- The picker offers HSV Rectangle, HSB Wheel, and HSV Circle. Select the primary
  or secondary swatch to edit it. Use the visual surface, numeric HSV controls,
  alpha slider, or a `#RRGGBB` / `#RRGGBBAA` hex value. The eyedropper samples the
  active cel. Left tools use primary color; right tools use secondary color.

| Shortcut | Action |
| --- | --- |
| X | Swap primary and secondary colors |
| Hold Space or Enter | Temporarily pan with a pointer drag |
| Middle-button drag | Pan |
| Mouse wheel | Zoom around the cursor |
| Ctrl+wheel | Increase/decrease the left tool size |
| Ctrl+Shift+wheel | Increase/decrease the right tool size |
| Ctrl/Cmd+Z | Undo |
| Ctrl/Cmd+Shift+Z | Redo |
| Ctrl/Cmd+A | Select the whole artboard |
| Ctrl/Cmd+D | Deselect |
| Ctrl/Cmd+1 | Open Tools sidebar |
| Ctrl/Cmd+2 | Open Layers sidebar |
| Ctrl/Cmd+3 | Open Timeline sidebar |
| Ctrl/Cmd+4 | Open AI chat sidebar |
| Escape | Cancel a gesture and clear the selection |

The zoom tool zooms in on click; right-click or Alt-click zooms out. Its settings
include Fit artwork and 100%. Shortcuts leave text and numeric fields available
for ordinary typing.

Selections use animated black-and-white borders and eight screen-sized resize
handles. With a selection tool, drag inside the selected area to move only the
boundary, or drag a handle to resize it. The sidebar exposes X, Y, Width, and
Height through Frame scrub fields. These operations transform the selection
mask with nearest-neighbor sampling; they never scale or edit artwork. Masks
retain holes and disconnected areas, including combined and polygon selections.
Selection masks are clipped to the artboard.

Tool-size outlines follow the pointer throughout the canvas workspace, including
outside the artboard. New selection outlines, resize handles, and dragged
selection boundaries can extend beyond the artboard during a gesture. The
preview retains the complete boundary until release, so dragging it outside and
back does not lose selected regions. Releasing clips the selection to the
artboard; a fully external drop leaves an empty selection. Pencil and eraser
strokes may enter from outside, but pixel edits always clip to the artboard.

The selection sidebar uses Frame's mode dropdown and Pixen's `ToolFieldPair`
component for Position (X/Y) and Size (Width/Height). Size's link toggle captures
the current aspect ratio; editing either field then updates both dimensions in
one boundary operation. Unlink to edit dimensions independently. The captured
ratio survives integer rounding and refreshes after an external boundary resize
or a new selection. Position is paired without a link. Deselect remains available
through Ctrl/Cmd+D; the sidebar contains no Deselect button or tool-help paragraphs.

Selection modifiers depend on when they are pressed. At the start of a new
selection, Shift chooses Add, Ctrl/Cmd chooses Intersect, and Ctrl/Cmd+Shift
chooses Subtract. Without modifiers, the sidebar's mode applies. The operation
stays fixed for the gesture, even if the keys change later. This works for
rectangle, ellipse, polygon, lasso, and color selections. Polygon mode is
captured at the first vertex; double-click completes the polygon.

Polygon tracing shows the committed vertices, a live edge to the pointer, and
the closing edge back to the first vertex. The start point is marked in the
application accent; approaching it after at least three vertices highlights the
marker and displays “Click to close.” Click there or double-click elsewhere to
complete the selection. Escape cancels tracing. `PolygonTrace` owns the reusable
vertex/preview/closure logic independently of selection masks and backend edits,
so future polygon shape tools can consume the same tracing interaction.

After starting a drag, pressing Shift constrains new rectangles/ellipses to a
square/circle, Ctrl/Cmd draws outward from the first pixel as the center, and
Ctrl/Cmd+Shift does both. Polygon and lasso geometry uses the same bounding-box
constraints while retaining its vertices. Keys already held when starting a
selection belong to the operation, so release and press them again to use them
for geometry. The preview updates immediately on key presses and releases,
including when the pointer is stationary. Selection operations can also be
chosen explicitly in the sidebar, including Intersect.

When dragging a handle, Shift preserves proportions and anchors the opposite
edge or corner. A side handle adjusts the perpendicular dimension symmetrically.
Ctrl/Cmd alone resizes freely around the center, mirroring the dragged edge or
corner on its opposite side. Ctrl/Cmd+Shift resizes proportionally around the selection's center. Without
modifiers, only the dragged edge or corner moves. The Move tool continues to
move selected pixels rather than just the boundary, and shows distance guides
to each artboard edge while dragging. Animation respects reduced-motion settings.
Holding the right mouse button also directs Ctrl+wheel to the right tool. Sizing
supports pencil, eraser, and text. Space pans without toggling animation playback;
use the timeline playback buttons to start or stop the animation.

## Drawing shapes

The toolbar includes Line, Curve, Rectangle, Ellipse, and Isometric box, each
assignable to either mouse button. These draw pixel outlines using that button's
color and independent per-shape stroke settings (size, density, opacity). Their sidebar shows
stroke controls. Previews do not modify the document; completing a shape creates
one undoable pixel edit, clipped to the active selection and artboard. Escape,
window blur, or changing the assigned tool cancels the draft.

Drag to draw a line, rectangle, or ellipse. Ctrl/Cmd mirrors the shape around
the initial point; Shift snaps lines to 22.5-degree increments around the full
circle, or constrains rectangles/ellipses to squares/circles. Combine Ctrl and
Shift for both effects. Hold Alt during a draft to displace its origin while
preserving its current geometry; outside shape drafting Alt retains color sampling.

Curve clicks place Bézier anchors; dragging an anchor sets its symmetric tangent.
The next segment previews under the pointer. Double-click completes the curve,
while Ctrl/Cmd removes the latest anchor. Isometric box uses four clicks: origin,
two base directions, and a height endpoint. Shift snaps base directions to
horizontal, vertical, or 2:1 isometric slopes and constrains the height vertically.
Ctrl/Cmd removes the last basis point for editing. Both staged tools show anchors;
curve tangent guides use the application accent.

L, J, S, and C assign Line, Curve, Rectangle, and Ellipse to the left button;
Alt plus the same key assigns the right button. Shortcuts leave text inputs alone.
`ShapeDraft` shares click tracing with polygon selections; `shapePoints` and
placement geometry provide tool-independent raster/constraint calculations.

## Verification

See [tool options](tool-options.md) for the complete tool/control audit,
assignment shortcuts, sampling behavior, and supported rendering options.

`bun test tests/frontend` covers raster operations and editor state.
`cargo test --test desktop_service` covers revision protection, undo/redo, and
document persistence. `scripts/verify-manual-tools.cjs` exercises browser gestures
against a running Vite server with mocked storage. Set `PIXEN_TEST_URL` to its
URL and run the script with Node and Playwright available. Native bridge compilation
is checked with `cargo check --manifest-path src-tauri/Cargo.toml`.

The drawing toolbar lives in a Frame dock inside the bottom of the workspace. Tools run left to right in a single scrollable row. Left-click and right-click assign each mouse button independently; the Tool settings sidebar keeps the color picker and per-button parameters.

Selecting a drawing tool or finishing an application menu command returns focus
to the canvas after menu dismissal, so holding Space or Enter can pan without
an extra artboard click. Text inputs, open menus, and dialogs retain their
keyboard interaction. Selection distance guides and polygon preview lines use
the active application's `--frame-accent` token; marching ants retain their
contrasting black-and-white border.

The Tools sidebar uses three Frame grouped views: Color picker, Left tool, and Right tool. Select tools only in the bottom toolbar. The picker places primary/swap/secondary, a one-shot artwork eyedropper, hash-free hex input, and picker-mode menu beneath its surface. Recent colors fill one horizontal row; Color options expands RGB/HSV channels and alpha. Undo/Redo live above each active document.

Color areas now use Frame's `rectangle`, `wheel`, and `circle` variants. Rectangle has a right-hand vertical hue slider; circle has a value slider. Recent swatches start empty and record only a valid committed hex entry or a successful non-empty pencil, bucket, paper, or text edit. Exploring the picker, sampling, moving, and erasing do not add colors. Entries deduplicate by RGBA and the latest 32 remain available for the width-fitting row. Frame's title-bar Back/Forward commands are routed to canvas undo/redo through its navigation adapter.

Picker types use a Frame dropdown; tool hints use Frame tooltips, and color models use Frame tabs (RGB, HSV, and linear RGB). Linear values are converted to/from sRGB while alpha is preserved. The swap action is a background-free Frame icon button.

The standalone eyedropper and held Alt display a moving 11-by-11 magnifier and sampled color. Drag to inspect and release the mouse to copy; releasing Alt restores the assigned tools. Escape or window blur cancels temporary picking. On Windows desktop, native capture samples visible application pixels, including the UI outside the artboard, with DPI conversion and app bounds. Browser preview samples artwork and flat DOM backgrounds; it does not capture rendered text or gradients. The magnifier is rendered as an OS cursor image, which native screen capture excludes. Merely sampling does not add recent colors.

Advanced color options are composed inside Pixen from Frame color sliders, numeric fields, and tabs. RGB uses 0–255 channel values; HSV uses hue degrees and saturation/value percentages; Linear uses 0–1 linear-light RGB. Alpha uses 0–255 in RGB/HSV and 0–1 in Linear. All rows update the selected color, hex field, picker, and other channels live. Intensity is exposure in stops (−8 to +8): linear-light RGB is multiplied by `2^I`, then encoded back to sRGB and clamped to the project’s 8-bit color range. Intensity is independent of the base color: the picker and RGB/HSV/Linear sliders always display and edit the unexposed base. Changing intensity updates only the final swatch, hex code, and paint color. Picker/channel/alpha edits and changing color-model tabs retain intensity. Left and right targets retain their own base color and intensity when switching between them. Entering a hex code or selecting a recent color establishes a new base with zero intensity. Returning intensity to zero restores the edited base without losing channels that clipped in the final output. Picker/slider exploration still does not populate recent colors until that color is painted or entered as hex.

The magnified eyedropper is exclusive to the standalone sidebar action and held Alt. It uses a custom cursor image with a centered crosshair, accent border and divider, and full-width color bar; no hex caption is displayed. Its backing canvas is Pixen-owned, not a Frame layout container. Native screen capture excludes the OS cursor, avoiding feedback when the preview covers the sample point. Toolbar eyedroppers remain canvas-only: they keep the pixel outline and copy into the assigned mouse button's color without a magnifier.

The magnified cursor uses half the theme control radius, capped at 5px. Its border and separator always use whichever of black or white gives greater contrast against the sampled center pixel.

Color channels are always visible below the recent-color row; the containing Frame view group controls collapse/height. Pencil and eraser size, density, and opacity use FrameScrubField: drag horizontally, use wheel/arrow steps, or click to type an exact value. Enter or blur commits direct entry; Escape cancels. Each mouse button retains its own brush parameters.

## Background saving and recovery

Undo/redo remains session-only memory. Desktop edits no longer write a recovery file or reload the document from disk; rendering consumes the current in-memory revision. Independent immutable revision snapshots let file writers operate while edits continue.

Settings → Recovery controls the recovery interval (1–3600 seconds, default 30). This preference is retained locally and applies to open and newly opened documents. The backend timer runs even when the window is minimized. Only changed revisions are written, with at most one recovery writer per document. Failed writes retain the preceding recovery file and report an error in the status bar. Completed checkpoints report “Recovery file updated”; they do not mark the project as saved. A crash can lose changes since the last completed checkpoint. Recovery contains artwork, not undo history.

Save / Save As captures a revision, writes it outside the editing/workspace locks, then marks only that revision saved. Later edits remain dirty. The status bar distinguishes “Saving project…” / “Project saved” from recovery. Closing waits for an outstanding manual save. Existing files are checked for external changes before replacement. Standalone headless MCP sessions retain their durable-per-transaction behavior.

### Timeline timing and drag feedback

Preview playback defaults to the document's saved frame timings. FPS overrides change preview playback only. Both the sidebar timeline and matrix let you double-click milliseconds to edit with FrameEditable; Enter or leaving the field applies a positive duration, while Escape cancels.

Layer and frame drags share a thumbnail/name preview in the sidebar and matrix. The complete matrix frame header includes its name/number and duration in hover feedback. The right status section shows preview timing, a separator, then the selected layer name or layer count followed by the number of selected cels.
