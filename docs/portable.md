# Storage modes and bundled AI skills

Create an empty file called `portable` (no extension) beside `pixen.exe`,
`pixen-cli.exe`, and `pixen-mcp.exe` before starting them. For example:

```powershell
New-Item -ItemType File .\portable
```

All three executables detect the marker beside themselves, regardless of the working directory.

| Mode | `portable` marker | Configuration, recovery, custom themes | Browser profile, cache, temp, logs |
| --- | --- | --- | --- |
| Default | Absent | Windows user folders | Windows user folders |
| Portable | Empty, or `portable` | `data/` beside the executables | `data/` beside the executables |
| Semi-portable | `semi-portable` | `data/` beside the executables | Windows user folders |

In **Settings > Installation**, choose a mode and **Apply and restart**. Pixen
flushes JSON settings and keyboard shortcuts, checkpoints open artwork, waits for
its window to close, and copies the affected folders. Conflicting destination
folders are renamed to `.pixen-backup-<migration-id>-<index>` beside their original
location. Original source folders remain untouched. The marker changes only
after copying succeeds. The helper verifies that the restarted application loads
its preferences; a failed restart attempts to restore the previous marker and
destination folders and reopen the checkpoint.

Open documents, their active tab, saved destinations and unsaved edits reopen.
Undo history and AI conversations reset. Close other Pixen windows and MCP
connections before applying; managed previews cannot change storage modes.
The application folder and destination must be writable. Copy failures leave the
previous storage active. Migration records and artwork checkpoints are retained
in `.pixen-migrations/` beside the app; they can contain personal artwork.
Backups and migration records are not included in release archives.

Manually creating or editing the marker only selects the next launch's storage
mode; it does not transfer data. Use Installation settings when switching an
existing profile. Close Pixen before editing the marker or moving its folder.

Portable `data/` contains `config/` (`settings.json`, `keybindings.json`, AI
configuration and model caches), `themes/`, `recovery/`, `webview/`, `temp/`,
`cache/`, and `logs/`. Settings are stored in JSON files; browser data is not
needed to retain those settings. The Installation page displays the exact
resolved locations. Existing custom themes alongside the executable remain
supported as a compatibility location.

Updating the application means replacing the shipped executable/resource files;
keep `data/`, `portable`, your custom themes, and backups. Do not replace the
whole extracted folder with a fresh one if it contains your profile.

Explicit project/export destinations remain where you choose them. External AI
provider tools, their sign-ins, and the WebView2 runtime remain external
prerequisites; portable mode does not redirect their homes or credentials.

The release includes the complete `skills/pixen/` and `skills/pixel-art/` folders,
with references and examples. Install or reference them through your AI client's
skill mechanism. Copying them into the release does not register them globally.

Release archives are assembled in a fresh staging directory and include only
shipped application files. They never include an existing portable `data/` folder,
a user's custom themes, or the optional marker from a previous extracted release.


## Settings file

Frame now saves application setting overrides in `settings.json` through native,
atomic file writes. Normal Windows mode uses
`%APPDATA%\com.poziel.pixen\settings.json`. Portable mode uses
`data/config/settings.json` beside the executables. Loading finishes before the
settings UI becomes available; pending writes are flushed before window close.

When the file is missing, Pixen imports the existing recovery interval, selected
theme, AI provider, and every provider's saved model from browser local storage.
The old keys are retained as a backup but are never used again while a settings
file exists. A malformed file is reported and left unchanged. Saved models do
not authenticate or test providers; model selection still requires a provider test.
Per-document comparison references and MCP-follow state are not global settings.

Older `data/EBWebView` profiles are relocated to `data/webview/EBWebView`
before the browser starts, preserving the one-time migration of legacy browser
preferences into JSON. No browser profile is deleted.

The `pixen.background` and `pixen.fps` settings are startup defaults for newly
opened or created artworks. Changing these preferences does not change any
artwork already open. Background and speed changes made through the current
artwork's view/preview controls remain local to that tab and do not rewrite the
defaults. Color mode is a per-artwork view control, not an application setting;
older global `pixen.representation` overrides are retired automatically.

Keyboard shortcut overrides use a separate `keybindings.json` beside
`settings.json` in both modes. Both pages have the same search, modal/tab, and
JSON controls. JSON changes apply only when saved; returning to the form
without saving discards the draft. Invalid files remain unchanged.
