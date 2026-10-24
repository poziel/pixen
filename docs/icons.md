# Editing Pixloom icons

Custom interface icons now live in `assets/icons/*.svg`. Edit or replace these
files directly; no Rust drawing code changes are needed. The running desktop
checks for edits about twice a second and reloads valid changes.

| Controls | SVG files |
| --- | --- |
| Menu and chat | `menu.svg`, `chat.svg` |
| Layer reset, visibility, isolation | `reset.svg`, `eye.svg`, `eye-off.svg`, `solo.svg` |
| Conversation options | `new.svg` (new chat), `copy.svg`, `revert.svg` (message actions) |
| Playback and frame navigation | `play.svg`, `pause.svg`, `stop.svg`, `previous.svg`, `next.svg` |
| Canvas zoom | `plus.svg`, `minus.svg` |
| Caption controls | `minimize.svg`, `maximize.svg`, `restore.svg`, `close.svg` |
| Composer and attachments | `attach.svg`, `send.svg`, `file.svg`, `close.svg` |

Keep the existing filenames. A square `viewBox="0 0 24 24"`, white strokes/fills,
and a 1.2px stroke match the current style. The UI tints icons for hover, disabled,
and selected states. Use paths/shapes rather than font-dependent text. Hit areas,
backgrounds, and layout remain in Rust; icon glyphs come from the SVGs.

In a source checkout the app reads `assets/icons` from that checkout. Builds also
copy SVGs to `icons/` beside the executables for use on machines without the source
checkout. Distribute that folder with the desktop. SVG loading/rasterization and
live texture caching are in `src/viewer_icons.rs`.

Egui still supplies standard widget affordances such as combo-box arrows and
utility-window close buttons. The application logo/window icon is separate:
`assets/pixloom.ico`, `assets/pixloom-48.png`, and other sizes and editable
`.pixloom` sources in `assets/`. See `assets/README.md` for branding assets.
