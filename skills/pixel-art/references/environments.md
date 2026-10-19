# Scenery, tiles and architecture

Start with [E1–E3 in sources](sources.md). The construction prompts below synthesize their environment lessons and practical review heuristics; they are not rigid rules for every camera or genre.

## Build a scene before polishing a tile

Block foreground, play space and background as broad masses. Decide where the player should look, what must be traversable or dangerous, and where UI will cover the scene. Check the composition with representative actors in place before texturing it.

Use overlap, scale and value grouping to establish depth. Keep passive detail from competing with interactive objects. A deliberately soft or low-contrast background can be successful even when a pickup needs sharp separation. Avoid assuming that every layer needs equally strong outlines.

## Tile construction

Choose metrics and projection from the game. Draw the center surface, boundaries, corners and transitions needed by the layout. Check each surface in a repeated patch: a tile can look attractive alone while producing obvious stripes or stamps in a map.

Continue clusters across wrapping edges. Look for dominant spots, accidental seams and tangencies. Introduce variants to solve visible repetition, not simply to increase asset count. Test concave corners, narrow corridors and transition intersections where relevant. Mirroring is useful only when it preserves lighting and intentional asymmetry.

## Natural objects and buildings

Construct trees as canopy volumes before individual leaves, rocks as planes before speckles, and buildings as coherent volumes before windows or bricks. Decide which forms should repeat quietly and which are landmarks. A striking asymmetric tree can work as a focal object and fail as a dense repeated forest stamp.

Keep the camera consistent across walls, roofs, props and actors. In isometric-style work, use a shared grid rather than estimating each slope independently. The common 2:1 pixel slope approximates true isometric projection; see S2.

## Diagnose in context

| Failure | Correction to try |
| --- | --- |
| Player disappears in foliage | Reduce competing contrast or create value separation around the play space |
| Ground reads as repeated dots | Redistribute dominant marks and connect larger texture clusters |
| Background feels flat | Establish overlapping depth masses before adding more detail |
| Decorative edge implies the wrong collision | Align visual cues with the playable boundary; test with an actor |
| Parallax shimmers | Inspect actual motion, scaling and sampling; revise the presentation as well as the art |

Review a composed scene, repeated tiles and likely camera positions. Do not auto-fit every environment object to the same apparent size.
