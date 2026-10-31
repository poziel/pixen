# Pixen Playbook

This folder bundles the instructions used by AI assistants working with Pixen: specialist agents, reusable skills and shared pixel-art rules. It gives client integrations one place to find the material they need for an artwork task.

## Folder structure

```text
playbook/
├── agents/    Specialist roles and responsibilities
├── skills/    Procedures for completing tasks
├── rules/     Shared craft rules, review criteria and pixel templates
├── LICENSE    License for the imported material
└── NOTICE.md  Attribution and import history
```

## Agents: who handles the work

[Agents](agents/) define specialist roles: what a specialist is responsible for, how it approaches the task and what it should return. Their Markdown files may also include client-specific model, effort or tool settings in frontmatter.

| Agent | Responsibility |
| --- | --- |
| [Animation director](agents/animation-director.md) | Plan poses, timing and animation structure. |
| [Palette smith](agents/palette-smith.md) | Choose, explain and repair palettes. |
| [Pixel critic](agents/pixel-critic.md) | Inspect artwork and report specific findings without editing it. |
| [Rig builder](agents/rig-builder.md) | Plan and build layers for independently animated parts. |

An agent describes a role. The client or agent runtime decides how to load that role and whether to delegate work to it; storing a file here does not spawn an agent or select a model automatically.

## Skills: how to complete a task

[Skills](skills/) provide repeatable workflows. Each skill has its own directory and a `SKILL.md` entry point, with supporting references or examples where needed.

Start with [studio](skills/studio/SKILL.md) for a task spanning several stages. It chooses workflows such as briefing, drawing, shading, rigging, animation, review and export. The [workflow index](skills/README.md) explains those choices.

Two additional bundles provide broader foundations:

* [Pixen](skills/pixen/SKILL.md) explains tool usage, document editing and evidence.
* [Pixel art](skills/pixel-art/SKILL.md) covers craft across subjects and asset types.

Skills describe the sequence of work and refer to shared rules rather than duplicating the full rulebook. They are instructions for using capabilities; they do not implement those capabilities themselves.

## Rules: what guides decisions and review

[Rules](rules/) contain shared pixel-art principles, technical guidance, review criteria and reusable grid templates. Their numbered filenames group related subjects: core principles, technique, color, characters, animation, creatures, environments, forms, effects and styles.

Begin with [core principles](rules/00-core-principles.md), load the subject files relevant to the task, and use the [review checklist](rules/07-review-checklist.md) to inspect the result. The [rules index](rules/README.md) describes the numbering.

Rules guide artistic decisions within the user's brief. Explicit project requirements and document-engine constraints must also be respected. Distinguish measurable requirements from artistic recommendations when reporting findings.

## How the three work together

For an animated character, the studio skill selects the rigging and animation workflows. The rig builder and animation director supply specialist planning where the client supports those roles. Each workflow consults the relevant layer, anatomy and timing rules. The pixel critic then reviews the artwork against the brief and the shared review criteria.

Keep role responsibilities in `agents/`, task procedures in `skills/`, and reusable craft knowledge in `rules/`. Cross-reference shared material when it applies to more than one workflow.

## Integration status

Pixen embeds a build-time catalog of the skills and rules in its binaries. Both standalone and desktop MCP connections advertise `skill://index`, `rules://index`, their referenced resources and workflow prompts named `pixen:<name>`. Embedded AI uses the same catalog through the app-approved `read_guidance` tool. Release packages also include this complete readable `playbook/` tree with its license and notice. Rebuild and reconnect to load changes; editing an installed Markdown file does not change the embedded catalog.

Use the running server's tool schemas and the [current MCP reference](https://github.com/poziel/pixen/blob/main/docs/mcp/mcp-reference.md) to establish what is available today. The [target MCP contract](https://github.com/poziel/pixen/blob/main/docs/specs/mcp-compatibility.md) defines the planned editing API refactor. Imported examples may require capabilities the running server does not implement. MCP prompts supply workflow instructions; they do not install host slash commands, upstream plugin extensions or specialist agents. Guidance does not authorize external tools or override application ownership of file actions.

See [NOTICE.md](NOTICE.md) for provenance and [LICENSE](LICENSE) for the imported material's license. Preserve attribution, source citations, procedures and pixel templates when adapting or packaging this bundle.
