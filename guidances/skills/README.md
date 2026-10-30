# Skills

Workflows an agent follows, in the order that catches mistakes while they are
still cheap. Each one is a procedure, not a description.

They are served from this one directory, under the same `pixen:<name>`
names everywhere:

- **Claude Code** loads them as plugin skills (`/pixen:draw`); **omp** gets
  the same commands from the plugin's omp extension.
- **Any MCP client** reads them as `skill://<name>` resources and can invoke
  them as MCP prompts named `pixen:<name>` — so Codex, Gemini CLI and Cursor
  get the same workflows.
- **The server instructions** list them, so an agent knows they exist without
  being told.

| Skill | Use when |
|-------|----------|
| `pixen:studio` | Any request — picks, orders and runs the others; the default entry point |
| `pixen:brief` | The request is open-ended and needs decisions before drawing |
| `pixen:concept` | Anything new: art spec → image-model prompt for a concept sheet or storyboard → import it as a reference |
| `pixen:new` | Starting a fresh document |
| `pixen:palette` | Choosing, building or repairing colours |
| `pixen:draw` | The main drawing work |
| `pixen:shade` | Flat art needs volume |
| `pixen:rig` | A character needs to be animatable |
| `pixen:animate` | Building a cycle |
| `pixen:tileset` | Level art and anything that repeats |
| `pixen:review` | Before saying anything is finished |
| `pixen:fix` | Editing art that already exists |
| `pixen:export` | Handing files to a game engine |

Skills reference `rules://` rather than restating craft, so a rule has exactly
one place to be wrong.
