# Integrated art conversation

Current refinement (2026-10-20): settings navigation is left aligned. A visible
bottom-right native resize grip and highlighted, wider edge targets improve
resizing. The attachment action has a flat, borderless gray hover style.

Conversation now aligns content-sized user bubbles right and plain artist text
left. Each message reserves a toolbar row whose time/actions appear only on hover.
Artist messages offer Copy; user messages offer Revert to this message. Reverting
restores the pre-request artwork and the Codex thread prefix, removes later chat,
and restores the original prompt/references to the composer without auto-sending.
Snapshots live in temporary files for the current connection, avoiding full
document history in RAM. Reconnect creates a fresh thread and invalidates its old
message controls. The title action is New conversation using external new.svg;
the earlier three-dot menu and turn-level revert action are superseded.

Reasoning summaries supplied by Codex appear in initially collapsed sections with
elapsed time. Timing continues while closed and freezes on completion or disconnect.
Raw hidden reasoning is neither requested nor fabricated. The isolated app-owned
MCP server explicitly approves its artwork tools; inherited integrations stay off.

Approved scope: optional Codex conversation beside the canvas, live MCP edits,
temporary recovery before mutations, Save As, Stop, revert turn, and new conversation.
Normal startup must not spawn an agent, initialize MCP, or inspect authentication.

UI refinement approved 2026-10-20: custom centered title bar with filename, main
menu and adjacent AI visibility toggle; left conversation; first message starts AI
and is queued until Ready; Enter sends and Shift+Enter adds a newline. The composer
uses restrained 6px corners and an inline Send/Stop action. Conversation actions
move into a title dropdown, file saves into the global menu, and Disable is deferred.
User messages use boxes and artist replies are plain text; consecutive messages
stack vertically with padding and no author tags or separators. The dark footer
shows canvas size left, timing right. The chat toggle opens the panel inward on
the left without changing window geometry. Flat title controls, file-path hover
tooltips, and small native Windows corners complete the chrome. New artwork and
Canvas size are non-AI document actions; resizing crops/pads from the top left.

The desktop owns one shared MCP Session after explicit activation. A token-protected
loopback bridge lets a small stdio proxy in pixen-mcp connect Codex to that session.
The external standalone MCP path stays unchanged. Embedded tools exclude document
replacement, export, finish, and opening additional viewers. New projects are created
in persistent temporary directories; recovery files survive shutdown. Existing files
remain untouched until an explicit Save As. Every edit writes recovery before commit.

Codex app-server runs as a hidden child over JSONL stdio. Authentication stays with
Codex. One thread holds successive turns; each turn captures an independent baseline.
Only one turn runs at a time. The GUI receives snapshots/events without waiting on
agent/network work. Stop interrupts; disabling AI terminates the child and listener.
Opening another document clears the document-bound conversation after stopping AI.

First pass has no direct API client, Claude adapter, credential storage, manual drawing,
or persisted chat restoration. Canvas navigation and inspection remain available.

Implementation authorization: user approved the complete prompt and requested direct
execution on a feature branch on 2026-10-20.

Further refinement: shared 40px panel headers, warm off-white primary text, gray
system text, inset artist replies, and activity-only footer status. Playback uses
Previous, Play/Pause, Stop, Next; Settings contains preview speed. The main menu
groups document actions, view options, background/color mode, inspection/review,
then Settings and Quit.

Settings refinement: searchable two-column preferences with Playback, Workspace,
and AI conversation sections. The exact default is GPT 6.1 Sol / low thinking.
Only visible models and advertised efforts are offered, using the CLI cache before
activation and model/list afterward. Per-turn model changes retain conversation
context; preferences are stored separately from Codex credentials. Canvas sizing
uses a reversible, immediate preview before explicit commit. All utility windows
have noncollapsible titles and roomier spacing; header icons use 1.2px strokes.
# Conversation refinement (2026-10-20)

- Short user bubbles fit their content; longer bubbles retain a right gutter.
  Artist text has a fixed left inset and a small right gutter.
- Consecutive messages from the same speaker share one timestamp when each gap
  is at most three minutes. User times align left, artist times align right.
- The growing composer places reference cards above the input, attachment action
  at bottom left, and Send/Stop at bottom right. References support raster images
  and UTF-8 documents; unsupported binary files produce an explicit error.
- Embedded MCP activity feeds the existing Follow AI bar. The footer shows app
  operations, while artist progress remains in the conversation/activity UI.
- Custom UI glyphs load from editable assets/icons SVG files. Header padding is
  shared between the layer and conversation panels.
