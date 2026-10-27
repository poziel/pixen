# Tool options audit

Tool names remain the first line of toolbar tooltips. Additional lines describe
left/right assignment shortcuts and the current tool's modifiers. The sidebar
contains controls, without help paragraphs. Dropdowns, checkboxes, text editing,
buttons, and number scrubbing use Frame components. Pixen composes the reusable
`ToolSelect`, `BrushToolOptions`, and `ToolFieldPair` wrappers.

| Tool | Key | Options and behavior |
| --- | --- | --- |
| Rectangular selection | R | Replace/add/subtract/intersect; position and linked size; established modifier ordering |
| Elliptical selection | M | Shared selection controls; circle and centered geometry modifiers |
| Polygonal selection | K | Shared selection controls; live outline; start-point and double-click completion |
| Lasso / free select | F | Closed freehand polygon; shared selection controls |
| Select by drawing | I | Square/round brush and size; select/deselect pixels; shared selection controls |
| Select by color | W | Global matching colors; tolerance; current-layer or merged-visible sampling |
| Magic Wand | Q | Connected matching region; tolerance; current-layer or merged-visible sampling |
| Move | V | Move selected artwork; copy instead of moving |
| Zoom | Z | Independent direction per mouse button; fit artwork; 100% |
| Pan | A | Pan or center artboard; temporary Space/Enter pan remains available |
| Text | T | Multiline text, font family, size, bold, italic, alignment, antialiasing |
| Color picker | O | Target mouse/left/right color; current/merged source; RGBA, RGB-only, alpha-only |
| Pencil | P | Square/round stamp, size, density, opacity, overwrite, solid/hollow stamp, stroke interior fill, linked X/Y spacing |
| Eraser | E | Square/round brush, size, density, opacity |
| Bucket | B | Connected/global/entire-selection fill; merged sampling; tolerance; color or transparent fill |
| Colored paper | — | Whole selection/layer fill with opacity |
| Line | L | Independent stroke settings; centered and 22.5-degree snap modifiers |
| Curve | J | Independent stroke settings; Bézier point tracing and tangent editing |
| Rectangle | S | Independent stroke settings; square/center/origin modifiers |
| Ellipse | C | Independent stroke settings; circle/center/origin modifiers |
| Isometric box | — | Independent stroke settings; four-point construction, snapping, basis correction |

Alt plus an assignment key chooses the tool for the right mouse button. Text
inputs and open controls retain their normal keyboard interaction. Alt alone
still samples colors outside a shape draft. Shape origin displacement retains
priority while drafting.

## Rendering details

Brush opacity/density remain percentages for consistency across Pixen. Overwrite
replaces the destination RGBA rather than blending it. Fill inside fills the
enclosed pencil path; Solid brush controls only the stamp interior. Spacing
inserts gaps between stamps, and linked gap fields starting at zero move together.
Shift can connect pencil/eraser strokes from their previous endpoint. A stroke
still applies opacity once per pixel and commits as one undo operation.

Tool options are independent per mouse button. Shape stroke settings are also
independent per shape tool. Selection mode and position/size continue to describe
the single shared selection. All pixel writes are clipped to the active selection
and artboard, including text, bucket fills, and shape drawing.

Merged sampling reads the visible composite but writes only the active layer.
RGB-only picking preserves the destination alpha; alpha-only picking preserves
the destination RGB. Text is rasterized on placement. Its family list uses local
fonts with browser generic fallbacks; it does not download fonts. Antialiasing
preserves fractional coverage when enabled and thresholds edges when disabled.

Selection mask resizing remains nearest-neighbor. cleanEdge and OmniScale are
not offered without their actual algorithms. Pattern/image brushes and font-file
import are not implemented by this audit.

## Verification

`bun test tests/frontend` covers brush masks, spacing, fill-inside, overwrite,
settings isolation, picker channels, metadata, text layout, and existing behavior.
`scripts/verify-tool-options.cjs` exercises all 21 panels and checks actual output
pixels, including a two-layer fixture for merged sampling. Existing drawing,
shape, selection, and toolbar browser suites remain applicable.
