# Prompt for the Frame project

Improve Frame's token documentation by generating complete references from the implementation, while preserving useful authored explanations.

Frame supports CSS variables and equivalent tokens in JSON, JSONC, JSON5, YAML, TOML and INI. A variable such as `--frame-text-input-padding` maps to `frame.text.input.padding`; nested, dotted and mixed object paths are equivalent. Inspect the current implementation rather than assuming historical token counts.

My goal is for every component's Styling documentation to describe both its own variables and shared variables that affect it. Apply the same approach to shell surfaces: title bar, navigation, view bars, sidebars, workspace/tabs, panels, status bar, menus and overlays. Give global foundations (colors, typography, spacing, radii, shadows and motion) their own reference.

Inspect `maintainer/component-documentation.md`, the existing component Markdown, `packages/core/src/themes/`, and the documentation extraction scripts first. Keep the existing Features/APIs/Styling component structure. Update outdated “CSS only” or “JSON allowlist” wording to reflect the current implementation. Call them tokens, regardless of file format.

Create one reusable extraction/metadata pipeline that produces:

- Component and shell styling references with token path, CSS variable, value kind, actual default/fallback, affected part/property/state, scope/limits, description and source.
- A complete searchable/grouped token catalog for humans.
- A machine-readable catalog for AI consumers, linked from `llms.txt`, with links to relevant documentation.

Capture direct usages and shared dependencies, including chains of `var()` fallbacks. Preserve context when one variable has different fallbacks across selectors. Distinguish authored descriptions from extracted facts; do not invent behavior from variable names. Keep unavoidable descriptive metadata in one maintained location rather than duplicating tables manually.

Explain which settings belong to tokens versus component props or Frame layout APIs. Runtime measurements and third-party geometry variables must not be presented as freely writable theme settings. Provide practical examples that change a shared foundation versus a component-specific override; the theme-format guide already covers serialization syntax, so do not repeat six format examples per token.

Add checks for missing variables, stale generated docs, incorrect token/CSS mappings, and broken reference links. Preserve current visual defaults, public compatibility, and unrelated uncommitted work. Do not commit or publish unless requested. Propose a concrete approach grounded in the current sources, then implement and verify the documentation pipeline.
