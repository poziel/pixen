# Desktop themes and views

Pixen's default theme is compiled from `assets/themes/pixen.yaml` into the app.
It is always available as **Pixen (built-in)** and cannot be replaced by an
external file. Changing the developer source requires rebuilding the application.

Custom themes belong in `themes/` beside the executable. On startup Pixen scans
regular files with JSON, JSONC, JSON5, YAML (`.yaml`/`.yml`), TOML, or INI extensions.
There is no manifest. Frame parses the contents and validates token names and
values; malformed documents, unrelated files, empty themes, duplicate theme IDs,
and reserved `pixen`/`pixloom` IDs are silently ignored. Files are limited to
128 KiB each, with up to 128 candidates loaded in filename order.

Valid themes appear in **Settings > Appearance > Theme**. Discovery does not
activate them automatically; selecting a theme applies it immediately and saves
the choice on this computer. Restart to discover newly added or edited files.
If a selected theme is removed or becomes invalid, Pixen uses its built-in theme.
`PIXEN_THEME` (or legacy `PIXLOOM_THEME`) can select a discovered theme ID or
filename at startup; arbitrary paths and registry aliases are no longer used.

The release includes `theme-examples/pixen-example.yaml`, an editable copy with
its own ID. Copy it into `themes/`, customize it, restart, and select it. Editing
the example cannot change the embedded Pixen default.

A minimal custom YAML theme looks like this:

```yaml
version: 1
id: my-theme
name: My theme
appearance: dark
tokens:
  frame:
    accent: "#FE725E"
```

Token-only documents are also supported; their filename stem becomes their ID
and display name. All application views use the same Frame tokens as the shell.
The canvas's checkerboard/black/white previews remain neutral to judge artwork.
Theme font-family values refer to installed fonts and do not load arbitrary files.

Frame's Codicon icons and Pixen's Lucide/inline SVG icons are bundled into the
frontend. No loose icon folder is read or shipped. Branding uses `assets/pixen.svg`
in the frontend and `assets/pixen.ico` in Windows executable resources.
`assets/fonts/Hack-Regular.ttf` is embedded for deterministic rendered labels.
