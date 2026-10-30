---
name: pixel-art
description: Draw, refine, and critique intentional pixel art for games. Use for pixel-art fundamentals, asset creation, style exploration, and visual cleanup across scenery, characters, creatures, items, icons, UI, animation, and effects. Combine with a project's specialized art direction and production tools.
---

# Pixel art craft

Make each cluster earn its place. Build readable shapes, give them convincing structure, then spend detail where it communicates identity or action. More pixels, colors, texture, or frames do not automatically improve the result.

## Start from the actual brief

Identify the asset's job, intended display size, source grid, projection, anchor, palette/lighting language, and neighboring artwork. Inspect supplied references and existing assets before inventing a style. Ask only for missing information that materially changes the work; otherwise state a reasonable assumption and proceed.

Separate **craft quality**, **fit for the game**, and **personal taste**. Flat, monochrome, rough, painterly, outlined, and detailed work can all succeed. Do not impose a tutorial's resolution, proportions, palette count, or favorite shading style as a universal rule.

## Load only what helps this asset

- Always use [fundamentals](references/fundamentals.md) when drawing or diagnosing pixel craft.
- For scenery, tiles and architecture: [environments](references/environments.md).
- For characters, creatures and portraits: [characters](references/characters.md).
- For inventory objects, equipment and props: [items](references/items.md).
- For symbols, controls, gauges and lettering: [icons and UI](references/icons-and-ui.md).
- For motion, projectiles and particles: [animation and effects](references/animation-and-effects.md).
- For choosing or analyzing a look: [styles and examples](references/styles-and-examples.md).
- For original human tutorials and provenance: [sources](references/sources.md). The user's preferred Juniper Dev video has its own [chapter reference](references/juniper-dev.md).

## Draw, inspect, correct

1. **Block the idea.** Start with a silhouette or rough outline and a few large color masses. Check recognition and pose at the intended size. Resolve geometry and negative space before decoration.
2. **Describe structure.** Separate overlapping parts and material regions. When shading is appropriate, choose lighting and shade volumes rather than tracing every boundary with darker rings.
3. **Resolve the grid.** Refine edge steps and meaningful clusters. Merge redundant colors and remove accidental marks. Use texture, dithering and manual anti-aliasing only where they support the chosen look.
4. **Test the asset's job.** Place it against likely backgrounds, beside related assets and inside its actual slot. For motion, inspect playback and the loop as well as individual frames. Revise the largest communication failure first.
5. **Deliver honest evidence.** Provide the editable authority and requested exports, identify what was visually inspected, record remaining limitations, and distinguish a concept or review candidate from accepted production art.

At minimum inspect native source scale, intended display scale and an enlarged nearest-neighbor view. Use enlargement to find pixel defects, not to substitute for readability testing. If rendering or visual inspection is unavailable, report that limit instead of claiming visual success.

## Work with tools without surrendering pixel decisions

Use controllable pencil, fill, selection, shape or scripted-grid operations. Automation is useful; inspect and refine its pixels. Generic gradients, blur, automatic resampling and random noise do not establish resolved pixel art. Downsampling a concept is a starting point, not evidence of finished craft.

Preserve meaningful editable parts, registration, transparency and frame timing. Use the consuming project's source format and export process; this skill is tool-independent. In Packbound, combine it with the applicable character/item/monster/UI/keyword art skill and spritex. Their dimensions, references and runtime rules remain authoritative. Keep unofficial work under `workspace/` and preserve original provenance.

## Give actionable critique

Describe the visible issue, its consequence, and a small concrete correction: “The hand merges into the hilt at display size; widen the grip separation before adding metal highlights.” Prioritize identity/pose, scene readability, form, then local pixel cleanup. Preserve intentional stylistic decisions and user-directed edits.

Use the human sources to explain techniques, not to fabricate artist endorsements. Do not claim to have watched a video, inspected an illustration or checked an animation unless you actually did. Reference examples teach transferable decisions; they are not permission to copy assets or adopt a new game style.
