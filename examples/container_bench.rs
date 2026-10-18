//! Reproducible local benchmark: cargo run --release --example container_bench
use anyhow::Result;
use pixloom::{
    container,
    encoding::{self, Kind, Placement},
    schema,
};
use serde_json::json;
use std::{io::Write, path::Path, process::Command, time::Instant};
fn peak() -> Option<u64> {
    #[cfg(windows)]
    {
        let out = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("(Get-Process -Id {}).PeakWorkingSet64", std::process::id()),
            ])
            .output()
            .ok()?;
        String::from_utf8(out.stdout).ok()?.trim().parse().ok()
    }
    #[cfg(not(windows))]
    {
        None
    }
}
fn worker(mode: &str, path: &Path) -> Result<()> {
    let start = Instant::now();
    for _ in 0..5 {
        match mode {
            "full" => {
                std::hint::black_box(pixloom::input::compile(path)?);
            }
            "frame" => {
                let mut r = container::Reader::open(path)?;
                let ids: std::collections::BTreeSet<String> = r.structure["layers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|l| l["timeline"][0]["cel"].as_str().map(str::to_owned))
                    .collect();
                for id in ids {
                    std::hint::black_box(r.decode_cel(&id)?);
                }
            }
            _ => unreachable!(),
        }
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / 5.0;
    println!("{}", json!({"ms":ms,"peak_working_set_bytes":peak()}));
    Ok(())
}
fn fixture(name: &str) -> Result<schema::Document> {
    let (side, count) = if name == "many-small" {
        (4, 2048)
    } else {
        (128, 32)
    };
    let canvas = schema::Canvas {
        width: side,
        height: side,
    };
    let mut seed = 12345u32;
    let mut cels = Vec::new();
    for n in 0..count {
        let mut pixels = vec![vec![0; side as usize]; side as usize];
        for (y, row) in pixels.iter_mut().enumerate() {
            for (x, p) in row.iter_mut().enumerate() {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                *p = match name {
                    "sparse" => {
                        if x == (n + y) % side as usize {
                            1
                        } else {
                            0
                        }
                    }
                    "rows" => (x % 4) as u32,
                    "tiled" => ((x % 8 + y % 8) % 4) as u32,
                    _ => (seed >> 24) % 4,
                };
            }
        }
        let kind = match name {
            "sparse" => Kind::SparseRows,
            "rows" => Kind::RowTable,
            "tiled" => Kind::Tiles { tile_size: [8, 8] },
            "animated" => Kind::HybridRle,
            _ => Kind::Grid,
        };
        let p = Placement {
            default_index: (name == "sparse").then_some(0),
            ..Default::default()
        };
        cels.push(schema::CelDefinition::Encoded(encoding::encode(
            format!("cel_{n:04}"),
            &pixels,
            &canvas,
            4,
            kind,
            p,
        )?));
    }
    let frames = if name == "animated" { count } else { 1 };
    let value = json!({"version":2,"canvas":canvas,"palette":["#00000000","#112233FF","#112233FF","#FFFFFFFF"],"cels":cels,"frames":(0..frames).map(|_|json!({"duration_ms":100})).collect::<Vec<_>>(),"layers":[{"id":"body","name":"Body","timeline":(0..frames).map(|n|json!({"cel":format!("cel_{n:04}")})).collect::<Vec<_>>()}]});
    schema::read_document(&serde_json::to_vec(&value)?)
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() == 3 {
        return worker(&args[1], Path::new(&args[2]));
    }
    let dir = tempfile::tempdir()?;
    for name in ["dense", "sparse", "rows", "tiled", "animated", "many-small"] {
        let doc = fixture(name)?;
        let bytes = serde_json::to_vec(&doc)?;
        let json_path = dir.path().join(format!("{name}.pixloom.json"));
        std::fs::write(&json_path, &bytes)?;
        let packed = dir.path().join(format!("{name}.pixloom"));
        container::write(std::fs::File::create(&packed)?, &doc)?;
        let mut baseline = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        baseline.start_file(
            "document.json",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .compression_level(Some(6)),
        )?;
        baseline.write_all(&bytes)?;
        let baseline = baseline.finish()?.into_inner().len();
        let measure = |mode: &str, path: &Path| -> Result<serde_json::Value> {
            let out = Command::new(std::env::current_exe()?)
                .arg(mode)
                .arg(path)
                .output()?;
            anyhow::ensure!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            Ok(serde_json::from_slice(&out.stdout)?)
        };
        println!(
            "{}",
            json!({"fixture":name,"canvas":doc.canvas,"cels":doc.cels.len(),"frames":doc.frames.len(),"json_bytes":bytes.len(),"split_zip_bytes":std::fs::metadata(&packed)?.len(),"single_document_zip_bytes":baseline,"json_full":measure("full",&json_path)?,"container_full":measure("full",&packed)?,"container_first_frame_indices":measure("frame",&packed)?})
        );
    }
    Ok(())
}
