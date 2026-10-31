# Rules

The shared pixel-art craft rules, indexed by `rules://index`:

- **MCP clients** read `rules://<filename-without-extension>` resources, such as `rules://00-core-principles`.
- **Pixen's embedded AI** uses `read_guidance` with the same URI, within the app-only tool policy.
- **Local readers** can use these Markdown files in the installed `playbook/rules/` folder.

Read `rules://index` for the list, or start at [core principles](00-core-principles.md). Files are numbered in bands: `0x` core discipline, `1x` technique, `2x` colour and materials, `3x` characters, `4x` animation, `5x` creatures, `6x` environments, `7x` objects and 3D, `8x` effects and UI, `9x` style. Subject files carry ` ```grid ` templates. The upstream rule-template renderer, tests and ADR are not shipped Pixen utilities. Guidance does not enforce palette snapping, shading or validation; use the running tool schemas to establish implemented capabilities.

Skills reference rules by name rather than restating them. MCP uses a build-time copy of this rulebook: rebuild and reconnect to load source changes. Editing an installed Markdown file does not change the embedded catalog.
