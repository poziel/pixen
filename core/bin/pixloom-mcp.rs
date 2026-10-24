use clap::Parser;

/// Run the Pixloom MCP server over stdio.
#[derive(Parser)]
#[command(name = "pixloom-mcp", version)]
struct Args {
    /// Automatically open the desktop inspector for editing projects.
    #[arg(long)]
    viewer: bool,
    /// Internal connection to the active desktop document.
    #[arg(
        long,
        hide = true,
        requires = "bridge_token",
        conflicts_with = "viewer"
    )]
    bridge: Option<String>,
    #[arg(long, hide = true, requires = "bridge")]
    bridge_token: Option<String>,
}

fn main() {
    let args = Args::parse();
    let result = match args.bridge {
        Some(address) => {
            pixloom::mcp::desktop_proxy(&address, args.bridge_token.as_deref().unwrap())
        }
        None => pixloom::mcp::run(args.viewer),
    };
    if let Err(error) = result {
        eprintln!("pixloom-mcp: {error:#}");
        std::process::exit(1);
    }
}
