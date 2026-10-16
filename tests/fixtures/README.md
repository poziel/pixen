# Fixture provenance

- `pixelorama-1.2.3-static.pxo`: lossless ZIP repack of the user's read-only
  `Downloads/l` extraction. Native metadata reports v1.2.3-stable / PXO 7,
  32x32, one raster layer/frame, 6 FPS. Includes the original native 256x256
  thumbnail. Raw layer SHA-256:
  `e44a6216beaf1746731db36273a426e15c522e080d040653936c3a0f8d90da3b`.
  Exactly 532 pixels have nonzero alpha. Archive SHA-256:
  `511b5c754934bc65fc07e0a62878b86b0a1efe8bf3bb67755e6a5ce1eb91be1c`.
- `authored-layers.json`: hand-checkable 2x1 input. Opaque red/empty base, half
  alpha blue with layer and cel opacity both 0.5, and a hidden green layer.
- `pixelorama-1.2.3-layers.png`: independently exported by the downloaded
  Pixelorama v1.2.3-stable from spritec's generated layered PXO, headlessly.
  RGBA bytes are `[223,0,32,255, 0,0,255,32]`. SHA-256:
  `a726c22ecfe9e220dfa2104f30c2ede5d4d1e180d6deb126a1c915d1385809b4`.
- `native-animation/animated.png` and `.json`: actual Pixelorama v1.2.3-stable
  exports of spritec's generated `skills/spritec/examples/animated.json` project,
  frames 1-4 as a 16x4 sheet. Native JSON confirms duration multipliers 1,2,1,1,
  tag 1-4, and links [0,2]. PNG SHA-256:
  `dd62634ec85607b4f5ea45e7491026de7b0f15a1d7644477284aa8167ca3e8f9`.

The original 640x400, 17-layer, 128-frame project from `Downloads/d` is exercised
locally through `SPRITEC_ANIMATION_FIXTURE`, without bundling its 2.2 GB of
expanded raster buffers. It was repacked without modifying content. The
independently exported native frame-24 PNG is supplied to the optional comparison
test using `SPRITEC_NATIVE_REFERENCE_PNG`.

Native exports were made on 2026-10-16 using the locally downloaded executable
and PCK. Raster comparisons do not rely solely on spritec writer/reader pairs.
