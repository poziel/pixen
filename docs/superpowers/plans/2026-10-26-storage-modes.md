# Storage modes and safe migration implementation plan

**Goal:** Implement Default, Portable, and Semi-portable, marker selection, and an Installation settings action that copies existing data and restarts safely.

**Architecture:** Core owns marker parsing, installation leases, and resolved paths. Native commands prepare a document checkpoint and transfer plan. A hidden helper invocation waits for the desktop/WebView to exit, stages copies, backs up conflicting destination directories, commits the marker last, and restarts with a readiness acknowledgement. Sources remain intact. No custom paths or bundled runtime are added.

**Decisions:** Empty `portable` means Portable; `semi-portable` contents select Semi-portable; absent marker selects Default. Mode is captured at process start. JSON settings and custom resources/recovery are durable. Browser profile/cache/temp/logs are local in Default/Semi-portable and beside the executable in Portable. Explicit project destinations stay unchanged. Other Pixen/MCP processes must close before migration. Open artwork reopens with its original destination and dirty state; undo history and chat sessions reset.

- [x] Core storage: marker modes, immutable process selection, paths, shared/exclusive installation lease; tests for marker parsing, semi-portable placement, and lock contention.
- [x] Artwork checkpoints: serialize open documents, original destinations and dirty/active state; restore without overwriting user projects; focused document test.
- [x] Native transfer: plan paths, stage/copy with retry for WebView shutdown, backup conflicts, marker-last commit, rollback and readiness acknowledgement; transaction tests for failure and conflicts.
- [x] Installation UI: Frame select/button, current locations and restart explanation; flush settings/keybindings, drain editor work, stop chats, prepare and restart; service/UI tests.
- [x] Verify: targeted Rust tests, frontend tests/type/build, native release, isolated real-process round trips Default -> Portable -> Semi-portable -> Default with unfinished art and conflicting destination settings; package updated release.

## Execution record

User approved the three-mode design and explicitly requested implementation. Preserve unrelated uncommitted work. Run inline using executing-plans; request one focused independent review after implementation.


## Verification completed

- Frontend typecheck/build and 113 frontend tests passed.
- Core Rust library: 47 passed, 4 ignored; native configuration/argument tests: 4 passed.
- Frame settings browser verification passed, including Installation presets and a recoverable migration refusal.
- Final production executable completed Default -> Portable -> Semi-portable -> Default with isolated APPDATA/LOCALAPPDATA. Both an edited saved document and an untitled document reopened dirty, the active artwork and JSON settings survived, destination conflicts were backed up, an active MCP lease refused migration, and saving afterward updated the original project path.
- Checkpoint regressions cover externally corrupted source files, missing source directories, original-file conflict protection and cleared undo history.
- Independent review findings addressed: restart-probation gate, restoration independent of decoding original files, checkpoint size checked before exit, missing artwork parent directories allowed during restore.
- Observed real Windows transfer failures reopened the source profile without changing original data. Copy tolerates browser temporary files disappearing during shutdown, retries directory activation, and Vite excludes native output/live profiles from watching.
- Release archive verified against all three current executable hashes. Shipped skills and storage instructions present; profile data, marker files and migration records absent.
- Output: target/portable/Pixen-0.2.0-x86_64-pc-windows-msvc.zip. Existing running release left untouched; a timestamped sibling release folder was produced.
