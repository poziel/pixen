# MCP activity example

Calls the real stdio MCP server to create a small sword/bow animation and
report user-facing activity through `report_activity`. No AI API or paid service
is used. All artwork and transcripts go into a fresh folder under `out/`.

Build all three executables, then run from the repository root:

```powershell
cargo build --locked --bins
python examples/mcp-activity/demo.py
```

The viewer opens automatically. While it draws, inspect the centered activity
and spinner, frame/layer following, and disabled controls. Switch **Follow AI**
off to inspect manually while updates continue, then on to resume following.
The finished preview stays open for five minutes; closing it is also safe.
At the end, the example saves its project and shuts down its MCP server.

For a fast endpoint check without a window:

```powershell
python examples/mcp-activity/demo.py --headless --delay 0 --hold 0
```

Every `report_activity` reply is checked to ensure it did not change the
project revision. `transcript.json` records actual MCP calls/responses;
`result.json` records success, and `mcp-stderr.log` records server diagnostics.
Use `--delay 4` for slower steps or `--hold 600` for a longer final preview.
