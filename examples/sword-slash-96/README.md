# Bronze duelist — Pixloom demo

Open **preview.html** locally for looping playback, frame stepping, speed control,
and seven independently toggleable layers. No server or dependencies required.
**preview.gif** is a 4× nearest-neighbour looping preview.

96×96 pixels, 10 frames, 7 layers, 57 cel definitions, 27 palette entries.
The animation lasts 1,050 ms: anticipation → slash → follow-through → recovery.
The palette uses binary alpha, so the transparent GIF needs no matte.

## Editable sources

- `duelist.pixloom.json`: optimized self-contained authoring source, 58,410 bytes.
- `duelist.pixloom`: deterministic split-cel ZIP container, 25,971 bytes (55.5% smaller).
- `duelist.pxo`: layered Pixelorama native project.
- `duelist.aseprite`: layered Aseprite native project.
- `generate.py`: dependency-free, authored palette-index geometry. `--force`
  explicitly regenerates the JSON; it does not regenerate exports automatically.

Layer order: ground shadow, scarf tails, legs/boots, armour/arms, sword,
head/scarf knot, slash trail/sparks. Tags label the full motion, windup, strike,
and recovery. Identical cel storage does not imply linked editing. The repeated
idle shadow has an explicit editing link; other repeated definitions stay
independent. Every definition is used, so the Aseprite build retains all artwork.

## Rendered files

- `duelist.png`, `.svg`, `.tiff`: transparent still of frame 1.
- `duelist.gif`, `.webp`, `.apng`: transparent 10-frame animation.
- `duelist.jpeg`, `.bmp`: opaque still of frame 1 on `#101B2E`.
- `duelist.mp4`, `.webm`: 1.05-second opaque animation on `#101B2E`.
- `strike.png`, `.svg`: transparent still of frame 6 (impact).
- `contact-sheet.png`: all 10 frames, 5 columns, native pixel scale.
- `contact-sheet-large.png`: the same sheet at 3× nearest-neighbour scale.
- `preview.gif`: a 4× nearest-neighbour looping GIF for easy viewing.
- `layers/*.png`: individual layer contact sheets used by the local preview.

WebM uses the existing lossless VP9 YUV444 pipeline, whose RGB conversion can
round colours. MP4 uses the existing lossless H.264 RGB pipeline. Both are opaque.

## Checks actually performed

See `verification.json` for file hashes, byte sizes and media probes.

- Both JSON and container pass `pixloom validate`.
- Container → unpacked JSON → repacked container produces identical archive bytes.
- Contact sheets rendered from `.pixloom`, `.pxo`, and `.aseprite` are byte-identical.
- FFprobe decodes 10 frames at 96×96 from GIF/WebP/APNG/MP4/WebM.
- GIF, MP4 and WebM report a 1.05-second duration.
- Contact sheet visually inspected for silhouette, layering, motion and canvas bounds.
- Preview opened in Chrome; pause, frame scrubbing, layer visibility and speed controls checked.

No existing artwork was renamed or modified. These are new local demo assets;
no consuming project was changed and nothing was published or committed.

## Reproduce exports from the repository root

```powershell
$cli = '.\target\release\pixloom.exe'
$demo = '.\examples\sword-slash-96'
# Use --force only when intentionally replacing this demo's generated outputs.
& $cli validate "$demo\duelist.pixloom.json"
& $cli pack "$demo\duelist.pixloom.json" --output "$demo\duelist.pixloom"
& $cli build "$demo\duelist.pixloom" --output "$demo\duelist" --formats pxo,aseprite
& $cli export "$demo\duelist.pixloom" --output "$demo\duelist" --formats png,svg,gif,webp,tiff,apng
& $cli export "$demo\duelist.pixloom" --output "$demo\duelist" --formats jpeg,bmp,mp4,webm --background '#101B2EFF'
& $cli render "$demo\duelist.pxo" --contact-sheet --columns 5 --output "$demo\contact-sheet.png"
```
