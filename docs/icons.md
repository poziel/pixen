# Pixen application icons

Application branding uses `assets/pixen.svg` for the Frame title bar and favicon,
`assets/pixen.png` for documentation, `assets/pixen.ico` for Windows executable
resources, and `assets/pixen.pixloom` as the editable source artwork.

The interface uses Frame's `FrameIcon` components (bundled Codicon glyphs),
`@lucide/vue` components for drawing tools, and application-owned inline SVGs.
These are bundled by Vite/Tauri. Pixen does not read loose SVG files at runtime;
the former `assets/icons` collection and its runtime copying step were removed.

Rebuild the frontend after changing interface icons. Rebuild the executables
after replacing `assets/pixen.ico`; running binaries retain their embedded icon.
