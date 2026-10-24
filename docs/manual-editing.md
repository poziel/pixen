# Manual pixel editing

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
| Escape | Cancel a gesture and clear the selection |

The zoom tool zooms in on click; right-click or Alt-click zooms out. Its settings
include Fit artwork and 100%. Shortcuts leave text and numeric fields available
for ordinary typing.
Holding the right mouse button also directs Ctrl+wheel to the right tool. Sizing
supports pencil, eraser, and text. Space pans without toggling animation playback;
use the timeline playback buttons to start or stop the animation.

## Verification

`bun test tests/frontend` covers raster operations and editor state.
`cargo test --test desktop_service` covers revision protection, undo/redo, and
document persistence. `scripts/verify-manual-tools.cjs` exercises browser gestures
against a running Vite server with mocked storage. Set `PIXLOOM_TEST_URL` to its
URL and run the script with Node and Playwright available. Native bridge compilation
is checked with `cargo check --manifest-path src-tauri/Cargo.toml`.
