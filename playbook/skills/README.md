# Skills

Workflows an agent follows, in the order that catches mistakes while they are still cheap. Each one is a procedure, not a description.

Pixen exposes these workflows from this directory:

- **MCP clients** discover `skill://index`, read `skill://<name>` resources and, where their host supports MCP prompts, retrieve prompts named `pixen:<name>`. A prompt supplies instructions; the agent then uses available tools.
- **Pixen's embedded AI** reads the same catalog through the app-approved `read_guidance` tool, without shell, plugin or external filesystem access.
- **Server instructions** advertise these entry points. Foundation bundles are resources at `skill://pixen` and `skill://pixel-art`; supporting text files use `skill://<name>/<relative-path>`.

MCP prompts do not install slash commands such as `/pixen:draw`. Hosts may display prompts in a command picker or offer their own skill installation mechanism; Pixen does not ship an upstream Claude/omp plugin. Tool schemas determine which workflow operations are implemented. Read a workflow's instructions without assuming every imported tool example is executable on the running server.

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

Skills reference `rules://` rather than restating craft, so a rule has exactly one place to be wrong.
