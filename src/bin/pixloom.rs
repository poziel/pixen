#![cfg_attr(windows, windows_subsystem = "windows")]

use clap::Parser;
use std::path::PathBuf;

/// Open pixel art projects in Pixloom.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Optional project file; omit to open an empty window.
    input: Option<PathBuf>,
    /// Internal stdio lifecycle used by the MCP server.
    #[arg(long, hide = true, requires = "input")]
    managed: bool,
}

fn main() {
    let args = Args::parse();
    if let Err(error) = pixloom::viewer::launch(args.input.as_deref(), args.managed) {
        eprintln!("pixloom: {error:#}");
        std::process::exit(1);
    }
}
