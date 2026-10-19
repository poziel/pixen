# MCP workflow

## Discover and open

Use the connected server's tool descriptions and input schemas for exact arguments;
this reference explains tool choice and sequencing. Host prefixes may change tool
names. Inspect available tools before relying on a capability: a running server
retains its old implementation until the client reconnects to a rebuilt binary.
If a tool is missing, report that limitation rather than inventing a call.

Use `open_project` for existing artwork or `create_project` for new artwork.
Use absolute paths and an existing destination parent. Prefer a `.pixloom`
destination; other supported project inputs can be opened, with non-Pixloom
inputs defaulting to a sibling `.pixloom` destination. Read returned IDs and
revision; use `get_project_info` to discover stable frame/layer IDs and timing.
Do not guess IDs or use timeline positions as IDs.

## Edit deliberately

Use `draw` for one drawing operation and `apply_edits` for a coherent atomic batch.
Use `edit_layer`, `edit_frame`, `edit_palette`, `copy_region`, and
`transform_region` for focused changes. Mutation calls use `expected_revision`;
carry forward the returned revision. On stale-revision errors, refresh state and
reconsider the edit before retrying. Invalid batches apply no partial changes.

Drawing coordinates are zero-based canvas pixels, strictly bounded; rectangles
use origin and positive dimensions. Use existing palette indices or explicitly
edit the palette. Setting a palette color can affect every use of that index;
check scope when remapping. Respect locked layers and intentional cel links.
Use `get_pixels` when exact index evidence matters. `undo` and `redo` support
iteration; they are separate from immutable review checkpoints.

## Review and improve

Choose evidence appropriate to the problem rather than running every review tool:

| Need | Tool and interpretation |
| --- | --- |
| Full frame or isolated layer | `inspect_project` or `inspect_layer`; inspect the returned PNG. |
| Readability across scales/backgrounds | `get_review_sheet`; choose color, silhouette, grayscale, and intended display scale. Native and enlarged evidence complement it. |
| Bounds, padding, colors, stray components | `analyze_sprite`; measurements and annotated components are observations, not automatic defects. |
| Before/after evidence | Create with `manage_checkpoint`, then use `compare_checkpoint`; interpret index, color, structural, and rendered changes in context. |
| Explicit technical constraints | `validate_artwork`; supply rules from the brief, such as size, palette, alpha, or protected baseline regions. `passed:false` is a completed check, not a tool failure. |
| Animation timing and transitions | `audit_animation`; include last-to-first for loops and choose thresholds only when justified. |

Create a checkpoint before substantial revisions. Checkpoint create/delete calls
require the current revision but do not advance artwork revision. Snapshots survive
undo and save, but end on `finish_project` or server exit. Save/export a separate
`.pixloom` copy for a durable baseline. At capacity, deliberately delete an unneeded
snapshot or use a file baseline; never assume checkpoints persist on disk.

After a focused correction, request fresh images and relevant comparisons/rules.
Reports describe their inspected revision; older evidence is not a check of new
artwork. Fewer changed pixels, fewer colors, isolated components, and identical
animation endpoints are not universal quality goals. Separate measured constraints
from subjective readability and style judgments. Limit correction rounds according
to the task and stop to report unresolved issues when progress stalls.

Review evidence can be large. Crop/isolate, reduce scales, or select fewer frames
when a resource limit is exceeded; do not describe an incomplete review as complete.
No tool automatically calls a critic model or creates a critic agent.

## Animate and inspect

Block out the subject, create distinct keyframes, add useful intermediate poses,
then inspect the contact sheet and playback before correcting and exporting.
`edit_frame` duplicates start as identical independent copies: draw changed poses.
Specify insertion `index` when timeline order matters and set `duration_ms`
deliberately, including intentional holds. There is no automatic tweening.

Use `get_contact_sheet` to examine pose order and timing; `audit_animation` adds
holds, distinct poses, bounds/centroids, and transition evidence. Use
`get_onion_skin` for neighboring poses and `compare_frames` for pairwise changes.
A static sprite repeated across frames does not satisfy an animation request.
`get_preview` shows only the first ordered frame.

PNG tool results provide AI visual feedback. `open_preview` opens the user's
inspector; opening a window alone does not mean the AI saw it or watched playback.
With Follow AI enabled, inspection/review calls synchronize the inspector's view;
with it disabled, manual viewing remains under the user's control. Onion skin,
frame comparison, and art reviews are inspection aids and do not edit artwork.

## Save and deliver

Use `save_project` to save without ending the session; `finish_project` saves and
releases it, ending its review checkpoints. Both refuse external disk changes.
Use `export_project` for explicit `.aseprite`, `.pxo`, `.pixloom`, JSON, or supported
image outputs without changing the project's save destination. Preserve originals.

Animated image exports use all frames unless a stable `frame_id` selects a still.
Set MCP `loop_animation:true` for indefinite looping; the CLI equivalent is
`--loop`. GIF may quantize colors and rounds timing to 10 ms. Video export requires
the CLI and FFmpeg. Report project/export paths, actual visual and rule checks,
and any relevant compatibility or unreviewed-playback limitations.

For repository development, consult `docs/mcp.md` for full API contracts and
`docs/art-review.md` for review semantics and budgets. Installed copies of this
skill should rely on live tool schemas when repository documentation is absent.
