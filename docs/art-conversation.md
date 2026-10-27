# Art conversation backend

The Frame/Tauri desktop currently includes an AI placeholder, not a conversation
frontend. The Rust adapter, subscription integration, attachments, approvals,
model preferences, checkpoints/rewind and cancellation remain available without
egui. Nothing starts AI automatically. The UI descriptions below document the
preceding egui implementation and are retained as reference for a future chat
frontend; they are not current desktop instructions.

Use the conversation toggle beside the main menu in the top-left title bar. The
conversation opens inward on the left, shifting the canvas into the remaining
space without moving or resizing the window. Codex starts only when you send the first message;
that message is retained during connection and sent once the art tools are ready.
Normal startup, opening files, and showing the panel do not launch AI or check login.

Install the Codex CLI and sign in with `codex login` in a terminal first. Keep
`pixen-mcp` beside the `pixen` executable; build/install all three Pixen
binaries together. Pixen uses the installed Codex App Server and its existing
credentials. Hover the conversation status for authentication information;
Pixen does not collect credentials or provide a direct API client.

Describe an asset, then send follow-up requests in the same conversation. Changes
appear in the current canvas. **Enter** sends; **Shift+Enter** inserts a newline.
While working, the composer action becomes **Stop**; completed edits remain.
The conversation title has a **New conversation** icon (`new.svg`). Hover a user
message to reveal **Revert to this message**. It restores the artwork from before
that request, removes that request and later turns from the Codex thread and chat,
and places the original text/references back in the composer for editing. Nothing
is resubmitted automatically. Artwork restoration is one undoable transaction.
Checkpoints use temporary files rather than retaining full document copies in RAM.
They belong to the current connection; reconnecting starts a new agent thread.
For an initially empty project, that baseline is the new transparent canvas.
Navigation and inspection remain usable while the artist works.

Before editing, existing artwork receives a separate recovery copy. New artwork is
created under the operating-system temporary directory (`%TEMP%\pixen-art-*` on
Windows). Each edit writes its recoverable `.pixloom.json` source before committing;
the temporary `artwork.pixloom` container is also updated. The footer reports current
application activity, including connection and saving status, without file paths.
Recovery files survive closing the application, but the
operating system can clean temporary storage: choose **Save As** to keep the artwork.

The top-left main menu contains **New artwork**, **Canvas size**, **Open file**, **Open Project**, **Save Project**,
and **Save As**. Creating artwork, resizing, and saving work without starting AI.
Canvas resizing keeps pixels anchored at the top left, crops smaller bounds, and
pads larger bounds with transparency. It does not scale pixels.
**Save As** selects a new `.pixloom` or `.pixloom.json` destination without restarting
the document or conversation. It refuses unrelated existing files. Subsequent **Save Project**
updates that chosen destination, refusing external changes. AI edits always update
recovery; permanent saves are explicit. The originally opened file stays untouched.
Choose a new destination for its AI-edited copy.

Hiding the conversation keeps the current agent and document session running.
Disabling AI is deferred. After a connection failure, sending a new message reconnects,
retaining the document and visible history but starting a fresh agent thread. **New conversation**
also starts fresh agent context without replacing the artwork. Opening another file
ends the current document-bound conversation; stop active work first. Chat is not
restored after application restart.

The desktop uses Rust with egui/eframe and an OpenGL renderer. Its custom title bar
centers the app name or filename, with native drag/minimize/maximize/close commands.
The dark footer shows the document's canvas dimensions at the left and frame timing
at the right. User messages appear in content-sized boxes aligned right;
artist replies appear as plain text aligned left with a small right gutter.
Consecutive messages stack vertically without author tags or separators.
The file title tooltip shows the full path; the application title has no tooltip.
Both side-panel headers have a 40px minimum height with centered labels and controls.
Primary text is warm off-white, with light-gray system text.
Windows uses small native rounded corners when supported by the system.

Previous/next frame buttons sit beside Play/Pause and Stop. **Settings** in the
main menu opens a searchable preferences window with a left category menu and
settings on the right. Playback contains preview speed; Workspace contains panel
visibility; AI conversation contains ChatGPT model and thinking controls.
Speed changes do not alter the saved frame durations. **Color mode** contains Color,
Silhouette, and Grayscale; analysis actions remain in **Review**. **Quit** is at the
bottom of the menu.

The embedded artist gets only the active-document art tools. Inherited MCP servers,
plugins, app integrations, shell execution and web search are disabled for its thread.
External MCP clients retain the standalone server's existing workflow. An externally
managed viewer cannot start a second embedded artist on the same source.

The first version targets the Windows desktop. Native Save As currently requires
Windows. Codex protocol/login/setup errors appear in the panel; hover a truncated
status for its full diagnostic. Unsupported structured agent requests fail explicitly
so the agent can ask in ordinary conversation instead.

The artist defaults to `gpt-6.1-sol` with `low` thinking. The model picker initially
uses the installed Codex catalog without launching a process; once AI connects,
it refreshes from App Server's `model/list`. Thinking options follow each model's
advertised capabilities. Model changes apply to the next message in the same
conversation. Preferences are stored in `%APPDATA%\Pixen\artist.json` on Windows,
separately from Codex login and global configuration. Opening Settings does not
initialize the artist. Protocol details: [Codex App Server](https://learn.chatgpt.com/docs/app-server).

Canvas size edits preview immediately without changing document pixels. **Resize
canvas** commits the bounds; **Cancel** or closing the dialog restores the previous
view. Dimensions use padded controls, and Settings, Canvas size, New artwork, and
Art review have no title-bar collapse toggle. Header icons share thin 1.2px strokes.

## Panel bounds and message times

The conversation has a 260px minimum width and a maximum of half the application
width. Window minimums adjust to visible panels and animation controls; with chat
and layers open, the base minimum is 640 x 380 logical pixels. With chat alone it
is at least 520 x 380. The activity bar and pending references increase the height
minimum to keep the composer usable. The Layers panel also reserves space for the canvas when resized.

Messages reserve a toolbar row below their content. Hovering reveals the local
HH:MM time plus Revert for user messages or Copy for artist messages. User tools
align right, artist tools align left. Times are light gray and outside the bubble;
streaming responses keep the time captured when their first content arrived.
Codex reasoning summaries appear in initially collapsed Thinking/Thought sections.
Their elapsed timer runs during reasoning and freezes when it completes, including
when the section is closed. Only summaries supplied by Codex are displayed. The Send/Stop control is
22px square. Layer and frame thumbnails use full square backgrounds with centered,
aspect-correct artwork and nearest-neighbor sampling.

AI edits use real MCP calls: the conversation starts Codex App Server, which launches
pixen-mcp as a stdio proxy to the app-owned authenticated loopback MCP server.
That server edits the shared Session and the app receives rendered document
snapshots. Manual controls call that same Session engine directly. resize_canvas
is now exposed through MCP as well.

## Reference attachments and composer

The composer grows with multiple text lines, scrolling the text area when needed,
with Attach at bottom left and Send/Stop at bottom right. It has no character
counter or model selector. Selected references appear as removable 96 x 88 cards
above the input. Images show a preview; text documents show a document icon.
Sent references also remain visible with the user's message.

The first pass supports raster images and UTF-8 documents (including code, JSON,
Markdown and SVG source). Attach at most eight files per message; images are capped
at 16 MiB compressed and bounded decoded dimensions, text documents at 1 MiB.
Unsupported binary documents are rejected with a visible error. Selected files
are snapshotted into app-owned temporary references; originals are not modified.
Images are sent as Codex localImage inputs and text content as reference text;
references can be sent without an accompanying prompt. Reading files or opening
the picker does not start Codex. Startup retains pending references until Ready.

## Embedded Follow AI

When the artist works, the existing AI activity bar appears, with a spinner,
centered activity text, and Follow AI toggle. Embedded MCP activity, inspection,
and review events use the same viewer paths as the external MCP workflow. Follow
can select the frame/layer being worked on; turn it off to inspect manually. Manual
controls unlock when the embedded turn finishes. The footer is reserved for app
operations such as Codex connection setup, saving and reference-file errors;
artwork activity and approval progress belong to the activity bar/chat instead.

Control glyphs are bundled Frame, Lucide, and inline SVG components; see [application icons](icons.md).

## Window and animation controls

Settings categories are left aligned. The window has a visible 20px bottom-right
resize target, larger side grab areas, and highlights the hovered resizing edges.
Maximized/fullscreen windows do not display or activate the resize helper.

The embedded MCP includes edit_frame, apply_edits, contact sheets, and animation
audits. Its app-owned server explicitly approves its scoped artwork tools through
Codex's [server approval configuration](https://developers.openai.com/codex/config-reference).
Inherited MCP servers, apps, plugins, and shell tools remain disabled for this
thread. No global Codex settings are changed. This avoids relying on inherited
approval defaults for frame-editing calls. A live subscription smoke test creates
two distinct frames and reverts both conversation and artwork to the first message.
