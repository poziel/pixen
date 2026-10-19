# Icons, UI and pixel lettering

Starting source: U1 in [sources](sources.md). Symbol discrimination, glyph checks and slice handling below are practical synthesis rather than attributed quotations. P1 is a discovery library for further topic-specific study.

## Icons communicate before they decorate

Identify the concept and its likely confusion partners. Draw a strong silhouette within the actual slot. Use negative space deliberately and maintain readable gaps. Test beside related icons at display size and in monochrome: color may reinforce meaning but should not be the only distinction.

Compare apparent weight and optical centering rather than forcing every shape to fill identical bounds. A sword, ring and flame may need different occupied areas to feel balanced. Keep the glyph separate from the button background, border, label and badge when the consuming control owns those elements.

If a small icon fails, first simplify its silhouette. Enlarging a complex object drawing and shrinking it back down is not a substitute for designing the symbol for its slot.

## Controls, panels and gauges

Use a coherent system for edges, corners, depth and states. Make enabled, selected, pressed, disabled and focused states distinguishable where relevant. Test actual combinations of text, icon and background; a beautiful empty frame may fail when populated.

If a frame uses slices, preserve corner dimensions and check stretched edges and repeated centers at several sizes. Avoid accidental fractional-pixel borders. A gauge must communicate amount or state quickly; decoration should not make full/empty or protected/unprotected regions ambiguous.

## Lettering

Test real strings and numbers, not just an alphabet sheet. Compare `I/l/1`, `O/0`, punctuation, signs and commonly adjacent glyphs. Check baseline, spacing and intended display scaling. Choose a supported font or appropriate glyph source rather than silently dropping required characters. Lettering is a specialized task; the current source set is not an exhaustive typography course.

## Quick corrections

- Similar symbols: change structure or negative space before relying on recoloring.
- Soft one-pixel strokes: inspect scaling and sampling before redrawing everything.
- Busy button: strengthen the symbol/background separation and reduce competing ornament.
- Unclear numeric state: test the actual label and value inside the control.

Judge the icon or control in context, including hover/focus/disabled appearances where applicable.
