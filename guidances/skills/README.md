# Skills

Workflows an agent follows, in the order that catches mistakes while they are
still cheap. Each one is a procedure, not a description.

They are served from this one directory, under the same `aseprite:<name>`
names everywhere:

- **Claude Code** loads them as plugin skills (`/aseprite:draw`); **omp** gets
  the same commands from the plugin's omp extension.
- **Any MCP client** reads them as `skill://<name>` resources and can invoke
  them as MCP prompts named `aseprite:<name>` — so Codex, Gemini CLI and Cursor
  get the same workflows.
- **The server instructions** list them, so an agent knows they exist without
  being told.

| Skill | Use when |
|-------|----------|
| `aseprite:studio` | Any request — picks, orders and runs the others; the default entry point |
| `aseprite:brief` | The request is open-ended and needs decisions before drawing |
| `aseprite:concept` | Anything new: art spec → image-model prompt for a concept sheet or storyboard → import it as a reference |
| `aseprite:new` | Starting a fresh document |
| `aseprite:palette` | Choosing, building or repairing colours |
| `aseprite:draw` | The main drawing work |
| `aseprite:shade` | Flat art needs volume |
| `aseprite:rig` | A character needs to be animatable |
| `aseprite:animate` | Building a cycle |
| `aseprite:tileset` | Level art and anything that repeats |
| `aseprite:review` | Before saying anything is finished |
| `aseprite:fix` | Editing art that already exists |
| `aseprite:export` | Handing files to a game engine |
| `aseprite:submit` | Sharing a finished sprite to the community gallery as a pull request |

Skills reference `rules://` rather than restating craft, so a rule has exactly
one place to be wrong.
