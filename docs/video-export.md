# Optional video export

`mp4` and `webm` render the complete authoring JSON timeline once. They require
at least two source frames. `--frame` is rejected when any requested output is a
video. Tags do not repeat or append frames. Neither output includes audio.

```powershell
pixen-cli export sprite.json --output assets/heart --formats png,mp4,webm --background '#203040FF'
pixen-cli export sprite.json --output assets/heart --formats mp4 --ffmpeg 'C:\Tools\ffmpeg.exe'
```

`--output` is an extensionless base path: these commands create `assets/heart.mp4`
and/or `assets/heart.webm`. In a mixed request, PNG uses frame 1 and video uses
every frame. Image exports and editable project builds work without FFmpeg.

## Containers and codecs

| Choice | Container | Encoder/settings | Pixels | Alpha | Dimensions |
| --- | --- | --- | --- | --- | --- |
| MP4 | MP4 | H.264 `libx264rgb`, `-crf 0 -preset ultrafast -pix_fmt rgb24 -bf 0` | Lossless rendered RGB, full colour resolution; decoded as planar GBR | None | Original canvas, including odd dimensions |
| WebM | WebM | VP9 `libvpx-vp9`, `-lossless 1 -pix_fmt yuv444p -lag-in-frames 0` | Lossless YUV 4:4:4; RGB/YUV conversion can round RGB channels | None | Original canvas, including odd dimensions |

No scaling, padding, chroma subsampling, interpolation, smoothing or intermediate
artwork is applied. MP4 prioritizes exact RGB over ubiquitous playback support:
H.264 RGB/4:4:4 lossless support varies between players and hardware decoders.
WebM also needs VP9 4:4:4 support. Universal browser/hardware playback is not
promised. Neither format preserves editable layers, tags, editing links or alpha.
Those belong in native project builds or an appropriate image output.

The renderer flattens layers first. If **any rendered frame** contains partial or
full transparency, supply an explicit background with alpha 255. Compositing
uses standard source-over RGB rounding. An opaque rendering needs no background;
hidden transparent cels alone do not require one. A supplied nonopaque background
is rejected even when the animation itself is opaque.

## Variable timing

Both outputs preserve variable frame durations. Pixen rounds each cumulative
timeline endpoint to the nearest millisecond, with positive half-milliseconds
rounded upward, then subtracts adjacent rounded endpoints. Thus total drift is
at most 0.5 ms rather than one rounding error per frame. A frame that collapses
to zero milliseconds is rejected with its frame number. Timelines longer than
24 hours are rejected.

Frames are rendered directly to temporary binary RGB PPM files; PNG is neither
an authoring requirement nor an encoding intermediate. A generated FFconcat
file uses portable relative filenames and a 1/1000 input time base. FFmpeg runs
with `-fps_mode passthrough -enc_time_base 1:1000`; MP4 also uses a track timescale
of 1000. The `setts` bitstream filter assigns each packet the interval to the next
timestamp and explicitly assigns the final packet its quantized duration.
There is no duplicate sentinel frame or constant-frame-rate conversion.

For example, durations `13, 127, 259` ms produce exactly three frames starting at
`0, 13, 140` ms and a 399 ms container duration. Thirty frames at `1000/60` ms
produce alternating 16/17 ms intervals and exactly 500 ms total playback.

## FFmpeg and output safety

Only requested video operations query FFmpeg. `ffmpeg` is searched through the
normal executable search path by default; `--ffmpeg PATH` selects another
executable. Relative executable paths are resolved before changing the encoder
working directory. Install a build containing the requested encoder and the
`concat` demuxer, PPM decoder, `setts` filter and corresponding muxer. A missing
program/encoder or an encoding failure is an error; requested files are never
silently skipped. Runtime exports require FFmpeg, while `ffprobe` is only used
by the external integration tests.

FFmpeg is launched through `std::process::Command` with separate arguments,
without a shell. All frame/list/output paths belong to a unique private working
directory. The module writes the video there, validates completion, and then
copies it to the caller's prepared file. The shared output installer handles
overwrite protection and installation after all requested outputs are prepared;
cross-file transactional atomicity is not promised.

Existing model limits still apply (4096 frames, 4096 per canvas side and at most
4,194,304 canvas pixels). Additional video limits are:

- At most 256 MiB for rendered temporary frames and their list, estimated before
  rendering. Identical source frames still count individually.
- Encoded outputs must be smaller than 128 MiB. FFmpeg receives `-fs` and the
  process runner watches output size; reaching the limit rejects the whole
  result, including when FFmpeg reports successful early termination.
- A 512 MiB temporary storage budget leaves headroom for the encoder's final
  packet beyond its `-fs` threshold. FFmpeg documents that `-fs` can overshoot
  slightly; a polling monitor is an additional guard rather than an OS quota.
- Captured stdout and stderr are each capped at 256 KiB; excess diagnostics are
  drained without accumulating them in memory.
- Encoder discovery has a 15 second timeout; encoding has a five minute timeout
  and one codec worker thread. Reduce canvas/frame count for exports exceeding
  these bounds.

Temporary files are removed on success and error. The library's
`VideoOptions::temp_parent` can choose an existing working parent for controlled
storage and cleanup tests. Failed encodes never copy their partial internal
output into the caller's prepared file.

## Verification

Regular tests cover missing FFmpeg, single-frame rejection, sub-millisecond
collapse, timeline/storage limits, nonopaque background rejection, transparency
in later frames and after compositing, unavailable encoders, simulated encoder
failure, output-limit failure, structured arguments with special-character
paths, unchanged prepared output on encoding failure and temporary cleanup.
The simulated encoder is a small executable compiled using the Rust toolchain;
these tests do not require FFmpeg.

Run the external codec/ffprobe/decode tests explicitly:

```powershell
cargo test --test video -- --ignored --nocapture
```

They check odd 3x3 dimensions, exact frame count and per-frame timestamps and
durations for both containers, 399 ms and 500 ms total durations, exact MP4 RGB
round trips, opaque background compositing, WebM's documented RGB rounding and
cleanup after successful exports. No external GUI editor/player checks are
included.

Verified on 2026-10-16 with FFmpeg and ffprobe 9.0.2: four regular tests and both
explicitly invoked external tests passed. The external tests decoded each MP4
frame back to the exact expected RGB values; the tested WebM RGB channels differed
by at most three integer levels after conversion. Other FFmpeg builds and player
compatibility need their own checks.

## References

- [FFmpeg codecs: libx264/libx264rgb and libvpx](https://www.ffmpeg.org/ffmpeg-codecs.html)
- [FFmpeg frame synchronization, encoder time base and file-size limit](https://www.ffmpeg.org/ffmpeg.html)
- [FFconcat demuxer, duration and per-file options](https://www.ffmpeg.org/ffmpeg-formats.html#concat)
- [setts packet duration expressions](https://www.ffmpeg.org/ffmpeg-bitstream-filters.html#setts)
