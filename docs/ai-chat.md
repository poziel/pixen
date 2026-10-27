# AI art chat

Choose a provider and model once in **Settings → AI art assistant**. Choices save
automatically, including a separate remembered model for each provider. Changes
apply to new conversations; existing conversations retain their provider/model.

Open or create artwork, select **AI chat**, and send a description. The first
message starts the provider connection and creates its conversation, then sends
that message. Opening the application, AI tab, Settings, or a new conversation
does not start a provider process. Stop also works while the first connection is
being established.

Model dropdowns use provider catalogs, not a fixed model list. Codex's installed
CLI cache can populate its dropdown before a connection. Other catalogs are
discovered from the first live connection and saved in `ai-models.json` in the
application config directory for future runs. Until a catalog is available,
**Provider default** works without selecting a model. Settings refreshes catalogs
only from existing connections or caches, preserving first-message-only startup.

**Settings → Chat appearance** controls the empty-chat message and the animated
working-border color. The border follows the theme accent by default; turn off
**Use theme accent for the working border** to choose an override. These
preferences save automatically and update the current view immediately. The
header's **Clear chat** sweep icon resets the conversation and AI context while
preserving the artwork.

The view embeds Frame's `AIChat` component from `@frame/features/chat`. The desktop
links the `frame-agent-connectors` crate from Frame Native. Supported app-only providers
receive the app's authenticated MCP server, which edits the same session as manual
tools. The canvas refreshes as edits arrive. Use the editor's Undo command to
undo artwork edits. Stop cancels the current response. Clear chat in the view header closes
the provider connection and keeps the artwork.

Each document has its own conversation for the current application run. Switching
documents retains the conversations; closing a document disposes its connection.
Opening the app or AI tab does not start a provider. Managed preview windows do
not offer AI editing. Native chat history cannot be rewound independently of the
provider, so message rewind/fork actions are hidden. Conversation transcripts are
not restored after restarting the app; no conversation-history browser is shown.

## Artwork context and references

Every message includes a fresh image of the bound document, even when no files
are attached. Animations use a contact sheet of up to the first 16 frames, with
dimensions, revision and frame count in the prompt. Overviews are bounded to
2048 pixels and enlarged with nearest-neighbor sampling for small pixel art.
Full-resolution detail and other frames remain available through MCP tools.

Use **+ → Files…**, drop files onto the composer, or paste an image to add
references. Files can be sent with or without message text. Images are labelled
separately from the current artwork so the assistant can distinguish references
from the canvas. PNG, JPEG, GIF, WebP, BMP and TIFF images are normalized to PNG;
UTF-8 documents, including code, JSON, Markdown and SVG source, are included as
reference text. Unsupported binary files produce an inline error.

Attach up to eight files, at most 16 MiB each and 24 MiB combined. Text files are
limited to 1 MiB; decoded images are limited to 8192 pixels per side and reduced
to a 2048-pixel overview for the provider. Originals are unchanged. App-owned
copies remain available during the conversation and are removed when its native
connection closes. Use a model that supports image input.

## Provider setup

Credentials stay with the installed tools:

- **Codex:** `codex` CLI with app-server support, signed in.
- **Claude:** Python 3.10+ with Frame Native's
  `agents/python/requirements.txt` installed and Claude SDK authentication set up.
- **Gemini:** `gemini` CLI with ACP support, signed in.
- **GitHub Copilot:** `copilot` CLI compatible with Frame Native's pinned SDK,
  signed in.
- **OpenCode:** an existing `opencode serve` process (default
  `http://127.0.0.1:4096`). Pixen does not stop this external server.

Optional native settings live in `ai-providers.json` in Tauri's application config
directory (`%APPDATA%/com.poziel.pixen` on Windows). This file uses Frame Native's
`RuntimeConfig` format. For example:

```json
{
  "connectors": {
    "claude": {
      "python": "D:/Tools/claude-env/Scripts/python.exe",
      "helper": "D:/Dev/1-codes/frame-native/agents/python/frame_claude_agent.py"
    },
    "opencode": { "endpoint": "http://127.0.0.1:4096" }
  }
}
```

Executable paths, arguments, environment, Python/helper paths, and OpenCode
authentication can be configured there. Pixen supplies the application identity,
isolated working directory and document MCP connection itself. It does not write
provider user configuration. Claude's default helper location uses the Frame
Native source checkout; set `helper` explicitly when distributing the app.

The app executable also handles `--ai-mcp-proxy` before starting Tauri. Provider
processes receive its bridge address/token through environment variables, keeping
them out of command-line arguments and the webview.

When building with an installed Copilot CLI, set
`COPILOT_SKIP_CLI_DOWNLOAD=1` to skip the SDK's optional bundled CLI download.


Large drawing calls use the compact `set_pixel_rows` operation and a recommended
200 KiB JSON call budget. See [Large AI drawing calls](mcp.md#large-ai-drawing-calls)
for strip batching, revision handling and undo behavior. Existing conversations
receive the budget reminder on their next message; reconnect to refresh MCP tool
schemas after updating the application.


## App-only actions and rendered replies

AI tool activity stays in the chat work timeline. The application status bar no
longer mirrors MCP activity labels. Activity summaries keep count and duration
together, for example `10 activities (27s, 2 failed)` and `Completed (1s)`.
Assistant answers and intermediate messages render Markdown, including tables,
lists, emphasis and code blocks. Raw HTML and remote image loading are disabled.

Pixen forces Frame Native's `appToolsOnly` policy on every conversation. The
working-directory restriction alone is not a filesystem sandbox. Codex removes
shell, local-image viewing, web, delegated agents, ambient MCP integrations and
plugins, with read-only execution as a second boundary. Claude disables built-in
tools and inherited settings and rejects non-Pixen tool permission requests.
Copilot uses an exact source-qualified allowlist of the document bridge's MCP
tools. OpenCode receives deny-by-default session permissions with only the
owned MCP server allowed, and must confirm those permissions before any prompt.
No provider-wide user configuration is changed.

Gemini ACP currently has no verified session-level built-in tool ceiling in this
connector. Pixen therefore refuses Gemini chat sessions instead of falling back
to unrestricted CLI tools. Its connector remains available to other Frame Native
hosts that do not request app-only actions. Provider model tests in Pixen use the
same policy, so Gemini reports this limitation in Settings.

The assistant may answer ordinary questions, inspect the attached artwork and
references, and edit the current canvas. It is instructed to decline external
actions. This is a provider tool restriction, not an OS-level sandbox around the
provider executable; authentication and model inference still use the provider's
normal network connection.
