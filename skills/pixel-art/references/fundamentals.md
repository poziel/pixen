# Core craft and correction guide

Practical synthesis, not universal style law. Source IDs resolve in [sources.md](sources.md). Apply the consuming project's art direction when techniques conflict.

## Draw from large decisions to small ones

Establish purpose, viewing size, camera and neighboring assets. Block the silhouette and major color masses. Resolve pose and volume before texture. Refine clusters and edges, then inspect at the actual display size. Repeat the weakest stage rather than adding detail everywhere. This workflow synthesizes F1–F3; an outline-first workflow is also valid.

## Observe, diagnose, correct

| Observation | Likely problem | Useful correction | Important exception |
| --- | --- | --- | --- |
| Object becomes unrecognizable when small | Competing details or weak silhouette | Preserve defining shapes; remove secondary marks | Some narrative illustrations reward close inspection |
| Outline has unintended bumps | Irregular step progression | Adjust segment lengths to support the intended curve | A deliberately rough surface needs irregularity |
| Narrow parallel shade strips trace the same contour | Banding exposes the grid | Reshape the shade masses around volume | A designed stripe is not automatically a defect |
| Middle is bright and every edge dark | Shading follows the flat boundary | Rebuild shadows from the object's forms and light | Frontal lighting is valid when forms are respected |
| Scattered bright dots compete with the subject | Texture overwhelms structure | Merge or remove marks that add no useful information | Eyes, sparks and selected highlights can require one pixel |
| Neighboring shades look identical in play | Palette distinctions carry little information | Merge shades or strengthen useful value differences | Subtle distant backgrounds may intentionally recede |
| Sprite looks good on one background, fringed on another | Edge colors assume a fixed background | Test on actual scene colors and revise the edge | Fixed illustrations may use background-specific edge treatment |
| Animation appears to crawl despite a stable pose | Detail changes without form movement | Track clusters across frames; stabilize nonmoving parts | Deliberate flicker may communicate a particular effect |

Cluster, banding, noise and pillow-shading diagnoses draw on F3. Palette/readability and form corrections also draw on F1–F2. Animation stability is a practical review heuristic rather than a claim extracted from these static tutorials.

## Make concrete pixel decisions

These editing exercises are synthesis for applying the techniques, not copied tutorial examples:

- **Contour:** inspect runs along a curve. An accidental short step between long runs can create a kink; move the boundary to make the progression fit the intended curvature. Do not force every organic edge into a perfect mathematical sequence.
- **Cluster:** if several disconnected dots are supposed to describe one fold or facet, try joining them into a readable shape. Keep a dot when it carries specific necessary information, such as a tiny eye.
- **Form:** sketch a box, cylinder or sphere underlying the surface. Place shadow on the plane turning away from the selected light rather than adding the same dark strip inside every outline.
- **Palette:** compare adjacent shades at display size. If they perform the same job, try merging them; if important planes disappear, separate their values. Hue shifts are an option, not an obligation to add colored rim lighting.
- **Texture:** reserve marks for characteristic surface information. Temporarily remove them; if the object reads better, rebuild fewer clusters around the form.
- **Simplification:** remove a secondary detail, then compare. Keep the version that communicates the asset's purpose more clearly, rather than assuming the busier drawing is more finished.

## Explain choices rather than enforcing a look

F1 uses volume, broad shading and restrained detail to build a sprite. F2 demonstrates simplification and recoloring before rebuilding forms. F3 emphasizes deliberate neighboring pixel groups and warns against uninformative isolated marks. None implies that all assets must be shaded, outlined, low-resolution or limited to a fixed color count.

Dithering can suggest texture or intermediate tones; whether it helps depends on scale and surface. Manual anti-aliasing can soften a transition but can also blur it. Colored outlines and broken outlines are different decisions, and authors use “selective outlining” differently. Inspect the actual boundary behavior rather than judging the label. See F1 and F3.

## Review at three levels

- **Pixel craft:** accidental edge breaks, clusters, palette behavior, transparency.
- **Asset communication:** identity, pose, material, direction and state.
- **Game communication:** separation from the background, consistency with neighbors and clarity during motion.

For a useful critique, state the visible issue, its player-facing consequence, the smallest useful correction and any stylistic tradeoff. “Bad pixel art” alone gives an agent no actionable information. Do not use technical file validation as evidence that the drawing succeeds visually.

## Preserve the result in presentation

P4 explains why integer enlargement with nearest-neighbor filtering preserves equal pixel sizes, while arbitrary reduction can destroy meaningful details. Judge source density separately from display size: enlarging a grid does not add drawn information. P2 provides an important qualification to blanket consistency rules: gameplay, UI and maps can use different treatments when those visual domains are intentionally coherent. Test the actual target presentation rather than declaring every mixed-resolution game incorrect.
