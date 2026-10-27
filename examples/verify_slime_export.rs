//! Verify a desktop GIF export from an actual artwork without modifying the source.
use anyhow::{Result, ensure};
use pixen::desktop_export::{ExportKind, ExportRequest, stage_batch};
fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    ensure!(
        args.len() == 2,
        "provide source artwork and a new GIF destination"
    );
    let source = std::path::Path::new(&args[0]);
    let destination = std::path::PathBuf::from(&args[1]);
    let document = pixen::input::read(source)?;
    let sprite = pixen::schema::compile(document.clone())?;
    let request = ExportRequest {
        mode: ExportKind::Animation,
        format: "gif".into(),
        frame: 0,
        scale: 5,
        grid_axis: "columns".into(),
        grid_count: 1,
        background: None,
        loop_animation: true,
    };
    for output in stage_batch(&document, &[request], &[destination.clone()])? {
        output.commit(false)?;
    }
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(std::fs::File::open(&destination)?)?;
    ensure!(
        u32::from(decoder.width()) == sprite.width * 5
            && u32::from(decoder.height()) == sprite.height * 5,
        "incorrect output dimensions"
    );
    let mut count = 0;
    let mut ticks = 0u64;
    let mut transparent = false;
    while let Some(frame) = decoder.read_next_frame()? {
        ensure!(frame.delay > 0, "zero frame delay");
        ticks += u64::from(frame.delay);
        transparent |= frame.buffer.chunks_exact(4).any(|p| p[3] == 0);
        count += 1;
    }
    ensure!(transparent, "missing transparent pixels");
    ensure!(count == sprite.frames.len(), "missing animation frames");
    ensure!(
        decoder.repeat() == gif::Repeat::Infinite,
        "missing infinite loop"
    );
    println!(
        "Verified {}: {} frames, {} × {}, infinite loop, {} ms",
        destination.display(),
        count,
        decoder.width(),
        decoder.height(),
        ticks * 10
    );
    Ok(())
}
