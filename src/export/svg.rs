use anyhow::{Result, ensure};
use std::io::Write;

// A checkerboard can otherwise expand a small raster into millions of XML nodes.
pub const MAX_RUNS: usize = 100_000;
pub const MAX_BYTES: u64 = 16 * 1024 * 1024;

fn runs(
    width: u32,
    rgba: &[u8],
    mut emit: impl FnMut(u32, u32, u32, &[u8]) -> Result<()>,
) -> Result<()> {
    let mut count = 0;
    for (y, row) in rgba.chunks_exact(width as usize * 4).enumerate() {
        let mut x = 0usize;
        while x < width as usize {
            let pixel = &row[x * 4..x * 4 + 4];
            let mut end = x + 1;
            while end < width as usize && &row[end * 4..end * 4 + 4] == pixel {
                end += 1;
            }
            if pixel[3] != 0 {
                count += 1;
                ensure!(
                    count <= MAX_RUNS,
                    "SVG exceeds {MAX_RUNS} visible pixel runs; use a raster format or simplify the image"
                );
                emit(x as u32, y as u32, (end - x) as u32, pixel)?;
            }
            x = end;
        }
    }
    Ok(())
}
pub(super) fn validate(width: u32, rgba: &[u8]) -> Result<()> {
    runs(width, rgba, |_, _, _, _| Ok(()))
}
pub(super) fn write<W: Write + std::io::Seek>(
    writer: W,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<()> {
    let mut writer = super::BoundedWriter::new(writer, MAX_BYTES)?;
    writeln!(
        writer,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" shape-rendering=\"crispEdges\">"
    )?;
    runs(width, rgba, |x, y, w, p| {
        write!(
            writer,
            "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"1\" fill=\"#{:02X}{:02X}{:02X}\"",
            p[0], p[1], p[2]
        )?;
        if p[3] != 255 {
            write!(writer, " fill-opacity=\"{:.9}\"", f64::from(p[3]) / 255.0)?;
        }
        writeln!(writer, "/>")?;
        Ok(())
    })?;
    writeln!(writer, "</svg>")?;
    Ok(())
}
