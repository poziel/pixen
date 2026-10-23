import { defineTheme, type Frame, type FrameThemeTokens } from "@frame/core";
import fallback from "../assets/themes/2026-dark.json";
export type ThemeConfig = typeof fallback & { font_base64?: string };
export function pixloomTheme(config: ThemeConfig) {
  const c = config.colors;
  const tokens: Partial<FrameThemeTokens> = {
    "--frame-bg": c.shell,
    "--frame-surface": c.panel,
    "--frame-canvas": c.canvas,
    "--frame-text": c.text,
    "--frame-muted": c.muted,
    "--frame-accent": c.accent,
    "--frame-line": c.border,
    "--frame-border-color": c.border,
    "--frame-list-selection-background": c.selected,
    "--frame-list-hover-background": c.hover,
    "--frame-button-primary-background": c.button,
    "--frame-button-primary-border": c.button,
    "--frame-menu-background": c.raised,
    "--frame-menu-border": c.border,
    "--frame-menu-hover": c.hover,
    "--frame-menu-disabled": c.disabled,
    "--frame-overlay-surface": c.raised,
    "--frame-font-size-body": `${config.metrics.font_size}px`,
    "--frame-font-body": config.font_base64
      ? '"Pixloom Custom", "Segoe UI", sans-serif'
      : '"Segoe UI", sans-serif',
  };
  return defineTheme({
    version: 1,
    id: "pixloom",
    name: config.name,
    appearance: "dark",
    tokens,
  });
}
export async function applyTheme(frame: Frame, config: ThemeConfig) {
  if (config.font_base64) {
    try {
      const bytes = Uint8Array.from(atob(config.font_base64), (c) =>
        c.charCodeAt(0),
      );
      const font = await new FontFace("Pixloom Custom", bytes).load();
      document.fonts.add(font);
    } catch {
      /* Use the system fallback if the selected font is unavailable. */
    }
  }
  frame.theme.set(pixloomTheme(config));
  for (const [name, color] of Object.entries(config.colors))
    document.documentElement.style.setProperty(
      `--pixloom-${name.replaceAll("_", "-")}`,
      color,
    );
  frame.layout.primaryWidth = config.metrics.sidebar_width;
}
export const defaultTheme = fallback;
