# Art conversation implementation plan

Goal: implement the approved [design](art-conversation-design.md) on
feature/art-conversation-panel, using Rust, egui, rmcp, and Codex JSONL stdio.

## Constraints and review focus
- No AI work on normal startup; explicit activation only.
- Recovery errors must prevent artwork commit; original files stay unchanged.
- Revert must retain stable IDs and increase revisions, even across many transactions.
- Stop, stale responses, disconnects, and pending approvals must not strand the UI.
- Embedded MCP must operate only on the active document and never open another viewer.

## Tasks
- [x] Session: add active identity, document snapshots, atomic baseline restore, and
  Save As with collision checks. Test external-file preservation and recovery failures.
- [x] Embedded MCP: share the existing Server, filter capabilities, authenticate a
  local bridge, and add stdio proxy mode. Test real MCP edits and rejected capabilities.
- [x] Codex adapter: launch lazily, handshake/authenticate, create thread, stream turns,
  interrupt, answer approvals, and clean up. Verify the protocol reducer, real bridge,
  installed-agent handshake and subscription-only two-turn smoke test.
- [x] Panel: activation, transcript, draft, status, stop, retry, new conversation,
  Save As and revert. Integrate snapshots with canvas without blocking GUI.
- [x] Verify: targeted tests, full cargo test, fmt and clippy. Document installation,
  recovery lifecycle and actual verification limits. Review the complete diff.

Execution: native in-session implementation, as directly requested by the user.

## Decisions and review

- Existing files are copied into a temporary editing workspace; permanent saves
  remain explicit. Save As refuses unrelated existing destinations.
- Disabling/reconnecting Codex retains the document session and its history.
  Reconnection starts a fresh agent thread and identifies that in the transcript.
- Full-canvas notifications are coalesced so a minimized UI cannot accumulate
  a queue of sprite copies.
- Native Save As is Windows-only, consistent with the existing native Open dialog.
- Independent read-only review identified interrupt-error handling, failed-revert
  retry, minimum-height panel overflow, and a process-wrapper cleanup race. Fixes
  close the connection on interrupt errors, retain baseline capability until a
  successful restore, pin the composer above a scrolling body, and start Windows
  processes suspended until assigned to a kill-on-close job.
- Embedded MCP instructions now match its restricted tool inventory.
- Live verification exposed deep-merge behavior and normalized nullable fields in
  App Server configuration overrides. Minimal disabled entries preserve inherited
  transports; a unique server name avoids collisions. The Apps feature is also
  disabled, and runtime tool inventory is checked before Send becomes available.
- Persisted chat, direct APIs, Claude, and artistic quality remain outside this pass.

## Verification

- `cargo test --locked --offline`: passed; environment-dependent live-agent,
  external-viewer and FFmpeg checks are opt-in.
- `cargo clippy --all-targets --locked --offline -- -D warnings`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Opt-in installed Codex initialization and MCP inventory check: passed with
  Codex CLI 0.163.0, using the existing login. No inference for this check.
- Opt-in two-turn art test: passed with subscription authentication; creates a
  4x4 sprite through MCP, refines it in the same conversation, checks exact rendered
  pixels, and reverts the second turn. The test refuses API-key authentication.
- Windows process-tree cleanup and egui layout at 280x260: passed automatically.
- Native window appearance and native Save As interaction need manual visual review.
- Work remains local on the feature branch, uncommitted and unpublished.

## UI refinement verification (2026-10-20)

- Custom title bar, left conversation panel, first-message startup, grouped
  messages, Enter/Shift+Enter behavior, and menu-based saving are implemented.
- Full offline suite: 220 passed, 9 opt-in checks ignored.
- Clippy with warnings denied, formatting, diff whitespace checks, and binary
  build passed.
- Regression checks cover title-bar dragging and controls, queued first-message
  delivery, and saving after an external file refresh.
- Native appearance and the Save As dialog still need manual visual review.

## Follow-up polish verification (2026-10-20)

- Flat chrome, normal title cursor, centered file-path tooltip, small native Windows
  corners, leftward window growth, simpler chat groups, and document sizing actions
  are implemented.
- Full suite: 223 passed, 9 ignored; the additional file-tooltip regression passed
  afterward. Final library run: 64 passed, 2 ignored.
- Clippy, formatting, whitespace checks, and all executable builds passed.
- Executables are in `target/ui-polish/debug` because the running application locks
  `target/debug/pixen.exe`. Native corner appearance still needs visual review.

## Panel, menu, and playback refinement verification (2026-10-20)

- Chat now opens inward without native window movement or resizing. Shared panel
  headers have a 40px minimum height with vertically centered content.
- Primary text is warm off-white; system copy is light gray. Artist replies have
  a left inset matching the right space beside user message boxes.
- The footer reports application activity instead of file paths. Main-menu groups
  follow the requested order, with Color mode, Settings, and Quit.
- Previous/next frame controls replace playback speed buttons. Settings changes
  preview speed without modifying saved frame durations.
- Full offline suite: 226 passed, 9 ignored. Clippy with warnings denied,
  formatting, whitespace checks, and the executable build passed.
- Automated UI checks cover inward toggling, matching panel header alignment,
  message insets, menu ordering, Settings interaction, and frame navigation.
- Updated executables remain in `target/ui-polish/debug`; native visual review
  remains manual. Work is local and uncommitted.

## Preferences and dialog refinement verification (2026-10-20)

- Settings now has search, left category navigation, and spacious right-side
  controls. Artist preferences default to gpt-6.1-sol / low, persist separately
  from Codex configuration, and apply to the next turn without clearing context.
- The picker uses the cached CLI catalog before activation and paginated live
  model/list results afterward, preserving advertised reasoning effort options.
- Menu, chat, reset, and conversation-options icons share 1.2px strokes. Utility
  window titles are noncollapsible; canvas and review controls have more spacing.
- Canvas width/height changes preview immediately without modifying artwork;
  Cancel restores the view and Resize canvas commits through the document session.
- Full offline suite: 230 passed, 9 opt-in checks ignored. The installed-Codex
  initialization/catalog check also passed separately, with no inference request.
- Final library suite: 70 passed, 2 ignored. Clippy, formatting, diff whitespace,
  and all binary builds passed. Updated executables are in target/ui-polish/debug.

## Thumbnails, panel bounds, message times, and MCP resizing (2026-10-20)

- Layer/frame thumbnails now pad aspect-correct nearest-neighbor artwork into a
  square background instead of presenting a narrow or stretched preview.
- Conversation width is 260px minimum and half the window maximum. Window minimums
  adapt to chat, layers, and animation; layer resizing reserves canvas space.
- User and artist messages show local HH:MM times beneath their content, aligned
  left/right in light gray. User timestamps remain outside message boxes. The
  Send/Stop action is 22px square.
- resize_canvas is exposed through standalone and embedded MCP. Both transports
  and the manual UI use the shared Session resize implementation with revision
  checks, exact palette-index preservation, recovery, and one undo step.
- Full offline suite: 238 passed, 9 ignored. Clippy, formatting, whitespace checks,
  and all executable builds passed. Native visual review remains manual.
- docs/icons.md maps the hand-drawn Rust UI icons and separate logo assets.
- Native visual review remains manual; changes remain local and uncommitted.

## Settings flow and dialog stacking fix (2026-10-20)

- Reproduced horizontal settings layout and zoom controls receiving clicks above
  windows with failing egui regression checks.
- Settings content now explicitly uses vertical layout within the sidebar row.
  Zoom controls render in the canvas UI layer instead of a foreground Area.
- Regression checks cover vertically stacked model/thinking and workspace fields,
  dialog draw order, and covered zoom controls rejecting pointer clicks.
- Library suite: 72 passed, 2 ignored. Clippy with warnings denied, formatting,
  whitespace checks, and all executable builds passed. The updated build remains
  in target/ui-polish/debug. Changes remain local and uncommitted.
## Conversation, references, activity, and SVG polish (2026-10-20)

- Content-fitting user bubbles, consistent artist gutters, and one left/right
  timestamp per consecutive speaker group with gaps no longer than three minutes.
- Growing composer with bottom attachment/send actions, removable reference cards,
  image previews, and UTF-8 document references. Binary documents remain unsupported.
- Reference snapshots survive asynchronous Codex reads. A live subscription-only
  smoke test correctly read a generated red image and the text key SLIME42.
- Embedded MCP activity now uses the Follow AI bar and inspection/review events;
  completed turns unlock manual controls. The footer retains application statuses.
- Roomier panel headers and 21 editable, hot-reloaded SVG glyphs in assets/icons.
  Builds copy icons beside executables for distribution.
- Final serial offline suite: 244 passed, 10 ignored. The new live reference test
  separately passed. Clippy with warnings denied, formatting, whitespace checks,
  and executable builds passed. Parallel full runs encountered intermittent
  Access denied in the existing recovery-history test; isolated and serial runs
  passed. Recovery implementation was unchanged in this refinement.
- Native visual review remains manual. Build: target/debug/pixen.exe; the older
  running ui-polish executable was left untouched. Local and uncommitted.

## Message tools, reasoning summaries, and native resizing (2026-10-20)

- Left-aligned settings categories; 20px corner resize grip, larger edge targets,
  and hovered-edge highlights. The paperclip uses a flat, borderless gray hover.
- Right-aligned, content-fitting user bubbles and left-aligned artist messages.
  Message times/actions appear on hover with a permanently reserved toolbar row.
  Artist messages have Copy; user messages have Revert to this message.
- Revert uses Codex thread/revert beforeTurnId plus the desktop pre-turn snapshot,
  then restores the selected text/references to the composer for editing. Snapshot
  files are temporary and connection-scoped. No automatic resubmission occurs.
- Initially closed reasoning-summary sections show elapsed duration, which freezes
  at completion/disconnection. Only Codex-supplied summaries are displayed.
- New conversation now uses external new.svg; message actions use copy.svg and
  revert.svg. The former title dropdown and turn-level revert action are removed.
- Artwork MCP tools have an explicit approval mode on the isolated embedded server;
  inherited integrations remain disabled and global Codex config is untouched.
- Live subscription test passed: create two distinct 4x4 animation frames through
  MCP, then revert the Codex thread and artwork to the original single blank frame.
- Final serial offline suite: 248 passed, 11 ignored. Clippy with warnings denied,
  formatting, whitespace checks, and executable builds passed. Native visual review
  remains manual. Updated executable: target/debug/pixen.exe.
- Changes remain local and uncommitted on feature/art-conversation-panel.
