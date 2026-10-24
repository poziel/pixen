# Migration verification

Verified on Windows against the Tauri executable built from this branch.

| Area | Evidence |
| --- | --- |
| Shell | Native WebView snapshot: one primary view bar with Layers, Timeline and AI placeholder; no document tabs or extra docked regions |
| Canvas | Native UI script loaded the six-frame/two-layer bounce example and received a 24×24 Rust render in the Vue canvas |
| Timeline | Native UI exercised pause/stop/next selection; frontend tests verify unequal frame timing and wrapping without editing durations |
| Layers | Native UI checked both layers and solo without advancing document revision; backend test checks transparent hidden-layer pixels |
| Create/resize/save | Service tests cover creation, resize, save/reopen, imported raster resize, external changes and Save As conflict recovery; new-artwork dialog exercised natively |
| View conversion | Service test switches a detected sprite strip to animation and back without changing revision, including MCP target-driven restoration |
| Inspection/review | Native UI calls analysis and opens the report; service tests verify review/preview preserve source bytes; frontend tests cover review isolation, pending targets/references and annotation invalidation |
| MCP | Opt-in real-desktop smoke test passes startup acknowledgment, edit/activity/inspection/review publication, reuse/focus acknowledgment, finish and EOF shutdown |
| AI backend | Existing backend tests pass with ordinary Wake callbacks and image attachments; live subscription tests were not run |
| Build | Vue type check, production frontend build and debug Tauri native build pass; native argument-contract test passes |

The full headless Rust suite recorded **194 passed, 0 failed, 12 ignored**.
Frontend verification recorded **5 passed, 0 failed**. The opt-in MCP desktop
test and native argument test each passed separately. Backend service tests were
also rerun after final changes. Ignored tests cover interactive/optional external
tools and live subscriptions; they are not claimed as verified.

The sandbox initially blocked eight baseline tests through temporary-file or
localhost socket permissions; the complete suite passed with those permissions
available. Vite reports a large-chunk advisory for the Frame bundle. The MCP
desktop test also emitted a Chromium window-class cleanup diagnostic while
exiting successfully. Neither is reported as a clean, warning-free run.

Screenshots/logs are generated under ignored `target/`. The repeatable native UI
procedure is in `frame-desktop.md`. Release installers and other operating systems
were not tested. The AI conversation frontend is intentionally absent; Frame
geometry and bundled SVG behavior replace egui-specific customization details
as documented in `viewer-themes.md`.
