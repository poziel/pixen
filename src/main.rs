use anyhow::{Result, ensure};
use clap::{Parser, Subcommand};
use pixloom::{
    export::ExportOptions,
    formats, native, operations,
    output::{self, Prepared},
    raster, schema,
    video::VideoOptions,
};
use std::{
    io::{self, Write},
    path::PathBuf,
};

#[derive(Parser)]
#[command(
    name = "pixloom-cli",
    version,
    about = "Export explicit pixel art and build editable native sprite projects"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    #[command(hide = true)]
    CanvasViewer { snapshot: PathBuf },
    /// Open a project in the standalone inspector.
    Preview { input: PathBuf },
    /// Import raster pixels as an editable Pixloom document.
    Import {
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        transparent_color: Option<String>,
        /// Maximum absolute RGB channel difference (0..255).
        #[arg(long, requires = "transparent_color")]
        color_tolerance: Option<u8>,
        #[arg(long, value_enum, requires = "transparent_color")]
        transparent_mode: Option<pixloom::import::TransparentMode>,
        #[arg(long)]
        trim: bool,
        #[arg(long)]
        force: bool,
    },
    /// Package authoring JSON as a compressed split-cel .pixloom container.
    Pack {
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        optimize: bool,
        #[arg(long)]
        force: bool,
    },
    /// Losslessly minimize JSON cel storage; destination is explicit.
    Optimize {
        input: PathBuf,
        #[arg(
            long,
            required_unless_present = "in_place",
            conflicts_with = "in_place"
        )]
        output: Option<PathBuf>,
        #[arg(long, conflicts_with = "output")]
        in_place: bool,
        #[arg(long, conflicts_with = "in_place")]
        force: bool,
    },
    /// Validate authoring JSON or .pixloom, including all pixels and editing links.
    Validate { input: PathBuf },
    /// Build editable projects from authoring JSON or .pixloom; append each format to OUTPUT.
    Build {
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value = "pxo")]
        formats: String,
        #[arg(long)]
        force: bool,
    },
    /// Export images/animation/video; OUTPUT is an extensionless directory/stem base.
    Export {
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value = "png")]
        formats: String,
        #[arg(long)]
        frame: Option<usize>,
        /// Loop GIF/WebP/APNG indefinitely. Static formats remain static.
        #[arg(long = "loop", conflicts_with = "frame")]
        loop_animation: bool,
        /// Opaque matte applied to exports: #RRGGBB or #RRGGBBFF.
        #[arg(long)]
        background: Option<String>,
        /// FFmpeg executable, used only for requested MP4/WebM outputs.
        #[arg(long, default_value = "ffmpeg")]
        ffmpeg: PathBuf,
        #[arg(long)]
        force: bool,
    },
    /// Print native metadata and unsupported features as machine-readable JSON.
    Inspect { input: PathBuf },
    /// Render actual cel data, with transparent unused contact-sheet cells.
    Render {
        input: PathBuf,
        #[arg(long, conflicts_with = "contact_sheet")]
        frame: Option<usize>,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        contact_sheet: bool,
        #[arg(long, requires = "contact_sheet")]
        columns: Option<u32>,
        #[arg(long)]
        force: bool,
    },
    /// Convert a .pixloom container or supported native project to the authoring JSON filename OUTPUT.
    Unpack {
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        force: bool,
    },
}
fn execute(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Preview { input } => pixloom::viewer::inspect(&input)?,
        Commands::CanvasViewer { snapshot } => pixloom::viewer::run(&snapshot)?,
        Commands::Import {
            input,
            output,
            transparent_color,
            color_tolerance,
            transparent_mode,
            trim,
            force,
        } => {
            let color = transparent_color
                .map(|c| -> Result<[u8; 3]> {
                    ensure!(c.len() == 7, "--transparent-color must be #RRGGBB");
                    let rgba = schema::decode_color(&format!("{c}FF"))?;
                    Ok([rgba[0], rgba[1], rgba[2]])
                })
                .transpose()?;
            let options = pixloom::import::Options {
                transparent_color: color,
                color_tolerance: color_tolerance.unwrap_or(0),
                transparent_mode: transparent_mode.unwrap_or_default(),
                trim,
            };
            for warning in pixloom::import::run(&input, &output, &options, force)? {
                eprintln!("warning: {warning}");
            }
            println!("imported {}", output.display());
        }
        Commands::Pack {
            input,
            output,
            optimize,
            force,
        } => {
            pixloom::container::pack(&input, &output, optimize, force)?;
            println!("packed {}", output.display());
        }
        Commands::Optimize {
            input,
            output,
            in_place,
            force,
        } => {
            let r = pixloom::optimize::run(&input, output.as_deref(), in_place, force)?;
            println!(
                "{} -> {} bytes (minified original: {}); JSON formatting/normalization: {} bytes; encoding savings: {} bytes; evaluated {} candidates, skipped {}",
                r.before,
                r.after,
                r.minified_before,
                r.before as i64 - r.minified_before as i64,
                r.minified_before as i64 - r.after as i64,
                r.evaluated,
                r.skipped
            );
        }
        Commands::Validate { input } => {
            let s = pixloom::input::compile(&input)?;
            println!(
                "valid: {}x{}, {} layer(s), {} frame(s)",
                s.width,
                s.height,
                s.layers.len(),
                s.frames.len()
            );
        }
        Commands::Build {
            input,
            output: destination,
            formats,
            force,
        } => {
            let formats = formats::parse_build(&formats)?;
            for path in operations::build(&input, &destination, &formats, force)? {
                println!("built {}", path.display());
            }
        }
        Commands::Export {
            input,
            output: destination,
            formats,
            frame,
            loop_animation,
            background,
            ffmpeg,
            force,
        } => {
            let formats = formats::parse_export(&formats)?;
            let background = background.as_deref().map(parse_background).transpose()?;
            let images = ExportOptions {
                frame,
                background,
                loop_animation,
            };
            let videos = VideoOptions {
                background,
                ffmpeg,
                ..Default::default()
            };
            for path in operations::export(&input, &destination, &formats, &images, &videos, force)?
            {
                println!("exported {}", path.display());
            }
        }
        Commands::Inspect { input } => {
            let report = if pixloom::input::is_container(&input) {
                pixloom::container::Reader::open(&input)?.structure
            } else {
                native::inspect(&input)?
            };
            let stdout = io::stdout();
            let mut lock = stdout.lock();
            serde_json::to_writer_pretty(&mut lock, &report)?;
            writeln!(lock)?;
        }
        Commands::Render {
            input,
            frame,
            output: destination,
            contact_sheet,
            columns,
            force,
        } => {
            output::preflight(&input, &[&destination], force)?;
            let s = if pixloom::input::is_container(&input) {
                pixloom::input::compile(&input)?
            } else {
                native::read(&input)?
            };
            let (width, height, pixels) = if contact_sheet {
                let columns = columns.unwrap_or((s.frames.len() as f64).sqrt().ceil() as u32);
                raster::contact_sheet(&s, columns)?
            } else {
                let f = frame.unwrap_or(1);
                ensure!(f > 0, "frame numbers start at 1");
                (s.width, s.height, raster::render(&s, f - 1)?)
            };
            Prepared::new(&destination, |file| {
                raster::png(file, width, height, &pixels)
            })?
            .commit(force)?;
            println!("rendered {}", destination.display());
        }
        Commands::Unpack {
            input,
            output: destination,
            force,
        } => {
            output::preflight(&input, &[&destination], force)?;
            let doc = if pixloom::input::is_container(&input) {
                pixloom::input::read(&input)?
            } else {
                schema::to_document(&native::read(&input)?)?
            };
            Prepared::new(&destination, |file| output::write_document(file, &doc))?
                .commit(force)?;
            println!("unpacked {}", destination.display());
        }
    }
    Ok(())
}
fn parse_background(value: &str) -> Result<[u8; 4]> {
    let colour = if value.len() == 7 {
        format!("{value}FF")
    } else {
        value.to_owned()
    };
    let rgba = schema::decode_color(&colour)?;
    ensure!(
        rgba[3] == 255,
        "--background must be opaque (#RRGGBB or #RRGGBBFF)"
    );
    Ok(rgba)
}
fn main() {
    if let Err(error) = execute(Cli::parse()) {
        eprintln!("pixloom-cli: {error:#}");
        std::process::exit(1);
    }
}
