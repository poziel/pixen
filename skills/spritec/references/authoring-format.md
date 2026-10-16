# Authoring format 1

UTF-8 JSON. Unknown fields are errors. The CLI never requires Pixelorama's
internal project metadata for new artwork.

| Field | Meaning |
| --- | --- |
| `version` | Required integer `1`. |
| `canvas` | Required `width` and `height`, positive integers. |
| `palette` | Required array of `#RRGGBBAA` strings; indices are zero-based integers. |
| `cels` | Required array of `{ "id": "name", "pixels": [[0,1], ...] }` definitions. May be empty for all-empty timelines. |
| `frames` | Required nonempty array of `{ "duration_ms": 125 }`. Every duration is finite and positive. |
| `layers` | Required nonempty ordered array of layers, **bottom to top**. |
| `tags` | Optional array of named inclusive frame ranges; defaults to `[]`. |
| `fps` | Optional finite positive export FPS; defaults to `12`. Durations still use milliseconds. |
| `annotations` | Optional project metadata/user-data; see below. |
| `pixelorama` | Optional native swatches/credits preserved by `unpack`; normally omit for new artwork. |

Each cel's `pixels` contains exactly `height` rows of exactly `width` numeric
indices. Index zero has no special meaning. Any palette entry may be transparent
or partially transparent. Hex case is immaterial. Palette entries are not
limited to 256: index `299` is valid with at least 300 entries. A palette contains
1..1048576 colours; palette indices are unsigned 32-bit integers.

Cel and layer IDs must be unique within their respective arrays. IDs and link
names contain 1..128 ASCII letters, digits, underscores or hyphens, with no
spaces or filesystem paths. A cel definition may be reused across layers or
frames. Unused definitions are valid, but a PXO round-trip may remove or rename
them because Pixelorama stores actual cels rather than reusable definitions.

## Layers and explicit timelines

```json
{
  "id": "body",
  "name": "Sprite body",
  "visible": true,
  "opacity": 1.0,
  "locked": false,
  "new_cels_linked": false,
  "timeline": [
    {"cel": "idle", "link": "held-idle", "opacity": 1.0},
    {"cel": "idle"},
    {"cel": "idle", "link": "held-idle"},
    {"cel": null}
  ]
}
```

`id`, nonempty `name`, and `timeline` are required. Visibility defaults to true;
opacity defaults to 1; lock/new-cel-link flags default to false. Opacity must be
finite and within [0,1]. A layer's timeline contains exactly as many entries as
`frames`. Entry position is its frame; arrays use ordinary zero-based JSON
indexing, while CLI frame numbers and tag ranges start at 1.

Every entry **must include `cel`**, either a known definition ID or JSON `null`.
Missing entries are errors, not holds. Null means a full-canvas transparent
empty raster cel. Entry opacity defaults to 1. There is no cel-visibility flag;
use opacity zero or an explicit empty cel where appropriate. Alpha is multiplied
by layer opacity and entry opacity; raw cel bytes remain unchanged.

`link` is optional. Entries with the same link name **within one layer** share
pixel edits in Pixelorama. Each group must reference the same definition ID (or
all null). A group may contain a single entry. Distinct names remain separate
groups even when their pixel grids are identical. Reusing `"idle"` without
`link` produces independent cels; the middle frame above can be edited alone.
Opacity/annotations remain per entry even in a linked group. Link names on
different layers never connect those layers.

`new_cels_linked` controls Pixelorama's default for frames added later. It does
not link any of the supplied timeline entries automatically. The existing
timeline is governed entirely by explicit `link` values.

## Timing, tags and repetition

```json
{
  "fps": 8,
  "frames": [{"duration_ms": 125}, {"duration_ms": 250}],
  "tags": [{"name": "idle", "from": 1, "to": 2, "color": "#6699CCFF"}]
}
```

Each frame's duration applies to all layers. Pixelorama stores
`duration = duration_ms / 1000 * fps`. Thus these frames use native multipliers
1 and 2 at 8 FPS. FPS does not round a frame to a whole tick. Millisecond values
may be fractional, and their native representation must remain finite/positive.

Tags require unique nonempty names and integer `from`/`to` satisfying
`1 <= from <= to <= frame count`. Tag ranges may overlap. Colour defaults to
`#6699CCFF`; optional `user_data` defaults to an empty string. There are no
ping-pong, playback direction, loop-count or repeat fields in version 1. To
repeat a sequence, write the repeated frame descriptors and timeline entries
explicitly, including any intentional edit links. Tags only label ranges.

## Annotations and native conversion

Projects, layers, frames and timeline entries accept
`"annotations": {"metadata": {"source": "hand-authored"}, "user_data": "notes"}`.
Both fields are optional and default to empty. Metadata is a JSON object;
keys beginning `_spritec` are reserved for internal provenance. Annotations are
stored as Pixelorama metadata/user-data and returned by supported conversions.

`pixelorama` optionally carries native `palettes` (an array of maps from palette
name to serialized native palette JSON), `project_current_palette_name`,
`license`, `author_display_name`, `author_real_name`, `author_contact`, and
`author_company`. Strings default to empty, palettes to `[]`. `unpack` preserves
these values so saved swatches and credits survive. They do not define the
numeric authoring palette or switch the project into indexed mode. New artwork
normally omits this section. Native palette objects are retained as data; Rust
does not evaluate Godot expressions or implement Pixelorama's palette UI.

When unpacking a native PXO, numeric palettes and grids are generated from exact
RGBA bytes. Identical grids may reuse a definition to reduce JSON size, without
creating editing links. Native link sets determine actual link relationships.
Layer IDs are retained where possible via provenance metadata; native duplicates
receive unique IDs. Independent native edits split reused definitions as needed.
Stale provenance never invents links. A linked set with inconsistent saved
pixel buffers is rejected rather than discarding any pixels.

## Limits and compatibility

Canvas: 1..4096 per side, at most 4 Mi pixels. Frames: 1..4096. Layers: 1..256.
At most 32768 layer/frame slots, 64 Mi pixels across definitions, 256 MiB decoded
definition bytes, and 256 MiB JSON. Output ZIP entry/size budgets can further
limit a build. Invalid grids, missing references, identifiers, links, ranges,
opacities, non-finite/zero durations, arithmetic overflow and unsupported fields
are errors. No allocations are made for repeated expanded timeline grids.

The supported subset preserves raw cel pixels and editing semantics. Normal
source-over rendering rounds once at output and can differ slightly from
Pixelorama's intermediate RGBA8 quantization. Nonzero z-index, groups, effects,
clipping and non-normal blends are rejected. Conversion does not preserve all
editor/UI/export settings or arbitrary archive entries. It is not an archival
round-trip guarantee.
