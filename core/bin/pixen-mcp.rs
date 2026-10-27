use clap::Parser;

/// Run the Pixen MCP server over stdio.
#[derive(Parser)]
#[command(name = "pixen-mcp", version)]
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
    if let Err(error) = pixen::storage::initialize() {
        eprintln!("Pixen storage: {error}");
        std::process::exit(1);
    }
    let args = Args::parse();
    let result = match args.bridge {
        Some(address) => pixen::mcp::desktop_proxy(&address, args.bridge_token.as_deref().unwrap()),
        None => pixen::mcp::run(args.viewer),
    };
    if let Err(error) = result {
        eprintln!("pixen-mcp: {error:#}");
        std::process::exit(1);
    }
}
