//! Optional FFmpeg video encoding. Image and native-project codecs do not use FFmpeg.

pub use crate::formats::VideoFormat;
use crate::{model::Sprite, output::BoundedWriter, raster, schema};
use anyhow::{Context, Result, ensure};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

const MAX_FRAME_BYTES: u64 = 256 * 1024 * 1024;
const MAX_OUTPUT_BYTES: u64 = 128 * 1024 * 1024;
const MAX_TEMP_BYTES: u64 = 512 * 1024 * 1024;
const MAX_CAPTURE_BYTES: usize = 256 * 1024;
const MAX_TIMELINE_MS: u64 = 24 * 60 * 60 * 1000;
const ENCODE_TIMEOUT: Duration = Duration::from_secs(300);
const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub struct VideoOptions {
    pub background: Option<[u8; 4]>,
    pub ffmpeg: PathBuf,
    /// Override the parent of the automatically removed working directory.
    pub temp_parent: Option<PathBuf>,
}

impl Default for VideoOptions {
    fn default() -> Self {
        Self {
            background: None,
            ffmpeg: PathBuf::from("ffmpeg"),
            temp_parent: None,
        }
    }
}

fn encoder(format: VideoFormat) -> &'static str {
    match format {
        VideoFormat::Mp4 => "libx264rgb",
        VideoFormat::Webm => "libvpx-vp9",
    }
}

/// Round cumulative endpoints, rather than each duration, to avoid accumulated drift.
fn durations(s: &Sprite) -> Result<Vec<u64>> {
    let mut elapsed = 0.0;
    let mut previous = 0;
    let mut result = Vec::with_capacity(s.frames.len());
    for (index, frame) in s.frames.iter().enumerate() {
        elapsed += frame.duration_ms;
        ensure!(
            elapsed.is_finite() && elapsed <= MAX_TIMELINE_MS as f64,
            "video timeline exceeds the 24 hour limit"
        );
        let endpoint = elapsed.round() as u64;
        ensure!(
            endpoint > previous,
            "video frame {} collapses at 1 ms precision; increase its duration",
            index + 1
        );
        result.push(endpoint - previous);
        previous = endpoint;
    }
    Ok(result)
}

fn frame_budget(s: &Sprite) -> Result<u64> {
    // Includes each PPM header and the ASCII concat entry, with ample header space.
    let bytes = u64::from(s.width)
        .checked_mul(u64::from(s.height))
        .and_then(|n| n.checked_mul(3))
        .and_then(|n| n.checked_add(256))
        .and_then(|n| n.checked_mul(s.frames.len() as u64))
        .context("video temporary frame size overflow")?;
    ensure!(
        bytes <= MAX_FRAME_BYTES,
        "video rendered frames exceed the 256 MiB temporary frame limit; reduce canvas or frame count"
    );
    Ok(bytes)
}

fn executable(options: &VideoOptions) -> Result<PathBuf> {
    // Preserve an explicitly relative executable when the encoder's working directory changes.
    if options.ffmpeg.is_relative() && options.ffmpeg.components().count() > 1 {
        Ok(std::env::current_dir()?.join(&options.ffmpeg))
    } else {
        Ok(options.ffmpeg.clone())
    }
}

/// Validate the rendered source, millisecond timeline, budgets and requested encoder.
/// FFmpeg is queried only when this optional video operation is requested.
pub fn validate(s: &Sprite, format: VideoFormat, options: &VideoOptions) -> Result<()> {
    schema::validate_model(s)?;
    ensure!(
        s.frames.len() > 1,
        "video export requires an animated source with at least two frames"
    );
    durations(s)?;
    frame_budget(s)?;
    if let Some(parent) = &options.temp_parent {
        ensure!(
            parent.is_dir(),
            "video temporary parent must be an existing directory"
        );
    }
    if let Some(background) = options.background {
        ensure!(background[3] == 255, "video background must be opaque");
    }
    for index in 0..s.frames.len() {
        let rgba = raster::render(s, index)?;
        ensure!(
            options.background.is_some() || rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255),
            "video frame {} has transparency; supply an explicit opaque background colour",
            index + 1
        );
    }
    let mut command = Command::new(executable(options)?);
    command.args(["-hide_banner", "-v", "error", "-encoders"]);
    let probe = run(&mut command, PROBE_TIMEOUT, None)
        .with_context(|| dependency_error(options, format))?;
    ensure!(
        probe.status.success(),
        "{}: {}",
        dependency_error(options, format),
        probe.stderr
    );
    ensure!(
        !probe.truncated,
        "FFmpeg encoder listing exceeded the 256 KiB diagnostic limit"
    );
    ensure!(
        probe
            .stdout
            .lines()
            .any(|line| line.split_whitespace().nth(1) == Some(encoder(format))),
        "{}",
        dependency_error(options, format)
    );
    Ok(())
}

fn dependency_error(options: &VideoOptions, format: VideoFormat) -> String {
    format!(
        "video needs FFmpeg with the {} encoder; install a suitable FFmpeg build or select its executable with --ffmpeg PATH (tried {})",
        encoder(format),
        options.ffmpeg.display()
    )
}

/// Encode one complete timeline into a private temporary output, then copy it to
/// the caller's prepared file. Never opens or installs a final destination.
pub fn write(
    writer: &mut File,
    s: &Sprite,
    format: VideoFormat,
    options: &VideoOptions,
) -> Result<()> {
    validate(s, format, options)?;
    let durations = durations(s)?;
    let frame_budget = frame_budget(s)?;
    let mut builder = tempfile::Builder::new();
    builder.prefix("pixloom-video-");
    let temp = match &options.temp_parent {
        Some(parent) => builder.tempdir_in(parent),
        None => builder.tempdir(),
    }
    .context("create video temporary directory")?;
    let mut concat = BoundedWriter::new(
        File::create(temp.path().join("frames.ffconcat"))?,
        s.frames.len() as u64 * 128 + 32,
    );
    writeln!(concat, "ffconcat version 1.0")?;
    for (index, &duration) in durations.iter().enumerate() {
        let rgba = raster::render(s, index)?;
        let rgb = raster::opaque_rgb(&rgba, options.background)?;
        let name = format!("frame-{index:04}.ppm");
        let mut frame = BoundedWriter::new(
            File::create(temp.path().join(&name))?,
            rgb.len() as u64 + 64,
        );
        write!(frame, "P6\n{} {}\n255\n", s.width, s.height)?;
        frame.write_all(&rgb)?;
        // Only generated portable relative names enter this FFmpeg data file.
        writeln!(
            concat,
            "file {name}\noption framerate 1000\nduration {}.{:03}",
            duration / 1000,
            duration % 1000
        )?;
    }
    concat.flush()?;
    drop(concat);

    let extension = match format {
        VideoFormat::Mp4 => "mp4",
        VideoFormat::Webm => "webm",
    };
    let output = temp.path().join(format!("encoded.{extension}"));
    let mut command = Command::new(executable(options)?);
    command.current_dir(temp.path()).args([
        "-hide_banner",
        "-v",
        "error",
        "-nostdin",
        "-n",
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
        "frames.ffconcat",
        "-map",
        "0:v:0",
        "-an",
        "-sn",
        "-dn",
        "-map_metadata",
        "-1",
        "-threads",
        "1",
        "-c:v",
        encoder(format),
    ]);
    match format {
        VideoFormat::Mp4 => {
            command.args([
                "-crf",
                "0",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "rgb24",
                "-bf",
                "0",
                "-video_track_timescale",
                "1000",
            ]);
        }
        VideoFormat::Webm => {
            command.args([
                "-lossless",
                "1",
                "-pix_fmt",
                "yuv444p",
                "-lag-in-frames",
                "0",
            ]);
        }
    }
    // No CFR conversion, sentinel duplicate, interpolation or frame truncation.
    // setts supplies *every* packet duration, including the otherwise 1 ms final packet.
    let duration_filter = format!(
        "setts=duration=if(eq(N\\,{})\\,{}\\,NEXT_PTS-PTS)",
        durations.len() - 1,
        durations.last().unwrap()
    );
    command
        .args([
            "-fps_mode",
            "passthrough",
            "-enc_time_base",
            "1:1000",
            "-bsf:v",
        ])
        .arg(duration_filter)
        .arg("-fs")
        .arg(MAX_OUTPUT_BYTES.to_string())
        .arg("-f")
        .arg(extension)
        .arg(format!("encoded.{extension}"));
    let encoded = run(&mut command, ENCODE_TIMEOUT, Some((&output, frame_budget)))
        .with_context(|| format!("FFmpeg {} encoding failed", encoder(format)))?;
    ensure!(
        encoded.status.success(),
        "FFmpeg {} encoding failed ({}); check the FFmpeg build/encoder: {}",
        encoder(format),
        encoded.status,
        encoded.stderr
    );
    let bytes = fs::metadata(&output)
        .context("FFmpeg did not create a video output")?
        .len();
    // -fs may exit successfully after truncating the timeline: never accept that output.
    ensure!(
        bytes > 0 && bytes < MAX_OUTPUT_BYTES,
        "video reached the 128 MiB encoded output limit; reduce canvas or frame count"
    );
    let mut input = File::open(output)?;
    std::io::copy(&mut input, writer)?;
    drop(input);
    // Explicit cleanup reports filesystem failures instead of silently relying on Drop.
    temp.close().context("remove video temporary files")?;
    Ok(())
}

struct ProcessOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
    truncated: bool,
}

fn capture(mut reader: impl Read) -> std::io::Result<(String, bool)> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    let mut truncated = false;
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let keep = count.min(MAX_CAPTURE_BYTES - bytes.len());
        bytes.extend_from_slice(&buffer[..keep]);
        truncated |= keep < count;
        // Drain excess diagnostics so a full pipe cannot deadlock the encoder.
    }
    Ok((String::from_utf8_lossy(&bytes).into_owned(), truncated))
}

fn run(
    command: &mut Command,
    timeout: Duration,
    output: Option<(&Path, u64)>,
) -> Result<ProcessOutput> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("start FFmpeg directly")?;
    let stdout = child.stdout.take().context("capture FFmpeg stdout")?;
    let stderr = child.stderr.take().context("capture FFmpeg stderr")?;
    let stdout = thread::spawn(move || capture(stdout));
    let stderr = thread::spawn(move || capture(stderr));
    let started = Instant::now();
    let outcome = loop {
        if let Some((path, frame_bytes)) = output {
            let bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            if bytes >= MAX_OUTPUT_BYTES || bytes.saturating_add(frame_bytes) >= MAX_TEMP_BYTES {
                break Err(anyhow::anyhow!(
                    "video reached its temporary/output storage limit; reduce canvas or frame count"
                ));
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Err(error) => break Err(error.into()),
            Ok(None) => {}
        }
        if started.elapsed() >= timeout {
            break Err(anyhow::anyhow!(
                "FFmpeg exceeded its {} second time limit; reduce canvas or frame count",
                timeout.as_secs()
            ));
        }
        thread::sleep(Duration::from_millis(20));
    };
    if outcome.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let stdout = stdout
        .join()
        .map_err(|_| anyhow::anyhow!("FFmpeg stdout reader failed"))?;
    let stderr = stderr
        .join()
        .map_err(|_| anyhow::anyhow!("FFmpeg stderr reader failed"))?;
    let status = outcome?;
    let (stdout, stdout_truncated) = stdout?;
    let (stderr, stderr_truncated) = stderr?;
    Ok(ProcessOutput {
        status,
        stdout,
        stderr,
        truncated: stdout_truncated || stderr_truncated,
    })
}
