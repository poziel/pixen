use clap::Parser;

/// Run the Pixloom MCP server over stdio.
#[derive(Parser)]
#[command(name = "pixloom-mcp", version)]
struct Args {
    /// Automatically open the desktop inspector for editing projects.
    #[arg(long)]
    viewer: bool,
}

fn main() {
    let args = Args::parse();
    if let Err(error) = pixloom::mcp::run(args.viewer) {
        eprintln!("pixloom-mcp: {error:#}");
        std::process::exit(1);
    }
}
