use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use spritec::{
    output::{self, Prepared},
    pxo, raster, schema,
};
use std::{
    io::{self, Write},
    path::PathBuf,
};

#[derive(Parser)]
#[command(
    version,
    about = "Compile explicit pixel-art JSON into editable Pixelorama PXO 7 projects"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Validate authoring JSON, including all pixels and editing links.
    Validate { input: PathBuf },
    /// Build PXO and a full-resolution PNG of frame 1.
    Build {
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        preview: PathBuf,
        #[arg(long)]
        force: bool,
    },
    /// Print PXO metadata and unsupported features as JSON (does not decode cels).
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
    /// Convert the supported PXO subset to OUTPUT/sprite.json.
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
        Commands::Validate { input } => {
            let s = schema::parse(&output::read_bounded(&input, schema::MAX_JSON)?)?;
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
            preview,
            force,
        } => {
            output::preflight(&input, &[&destination, &preview], force)?;
            let s = schema::parse(&output::read_bounded(&input, schema::MAX_JSON)?)?;
            let project = Prepared::new(&destination, |file| {
                pxo::write(&mut *file, &s)?;
                ensure!(
                    file.metadata()?.len() <= spritec::archive::MAX_ARCHIVE,
                    "PXO exceeds compressed archive limit"
                );
                Ok(())
            })?;
            let pixels = raster::render(&s, 0)?;
            let image = Prepared::new(&preview, |file| {
                raster::png(file, s.width, s.height, &pixels)
            })?;
            project.commit(force)?;
            image.commit(force)?;
            println!(
                "built {}; preview frame 1: {}",
                destination.display(),
                preview.display()
            );
        }
        Commands::Inspect { input } => {
            let report = pxo::inspect(&input)?;
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
            let s = pxo::read(&input)?;
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
            let sprite = pxo::read(&input)?;
            let doc = schema::to_document(&sprite)?;
            // Prepare in the parent first, so conversion failures never create the output directory.
            let parent = destination
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new("."));
            ensure!(parent.is_dir(), "unpack parent directory must exist");
            let mut staged = tempfile::NamedTempFile::new_in(parent)?;
            output::write_document(staged.as_file_mut(), &doc)?;
            staged.as_file().sync_all()?;
            let created = if destination.exists() {
                let meta = std::fs::symlink_metadata(&destination)?;
                ensure!(
                    meta.is_dir() && !meta.file_type().is_symlink(),
                    "unpack output must be a real directory"
                );
                ensure!(force, "unpack directory exists (use --force)");
                false
            } else {
                std::fs::create_dir(&destination).context("create unpack directory")?;
                true
            };
            let target = destination.join("sprite.json");
            let result = (|| -> Result<()> {
                output::preflight(&input, &[&target], force)?;
                if force {
                    staged.persist(&target).map_err(|e| e.error)?;
                } else {
                    staged.persist_noclobber(&target).map_err(|e| e.error)?;
                }
                Ok(())
            })();
            if result.is_err() && created {
                let _ = std::fs::remove_dir(&destination);
            }
            result?;
            println!("unpacked {}", target.display());
        }
    }
    Ok(())
}
fn main() {
    if let Err(error) = execute(Cli::parse()) {
        eprintln!("spritec: {error:#}");
        std::process::exit(1);
    }
}
