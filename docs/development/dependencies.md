# Frame dependencies

## Unreleased Tauri agent adapter migration

The current connector migration uses the sibling Frame Server `packages/agents-rust` crate with its optional `tauri` feature. Run `bun run deps:local` after building Frame's Tauri package so the new `@frame/tauri/agents` entry is available. Both sibling checkouts are currently required to build this revision.

The published dependency configuration described below is the previous release baseline. Before releasing this revision, publish the updated Frame Server crate and Frame Tauri package, replace the temporary Cargo path dependency and old npm pins with those released versions, and regenerate both lockfiles. The release dependency check must continue to reject local Cargo paths. Do not use `deps:released` or `build:released` to validate this unreleased migration. `PIXEN_FRAME_SERVER_PATH` cannot redirect the temporary Cargo path; the sibling layout is required until the release dependency pins are updated.

The wrappers support local and released dependency modes. During this migration, only local mode describes a usable build of this revision; the published pins below describe the previous release baseline.

## Published dependency baseline (before this migration)

- `@frame/core` aliases GitHub package `@poziel/frame-core@0.1.0-dev.4.1`.
- `@frame/tauri` aliases GitHub package `@poziel/frame-tauri@0.1.0-dev.4.1`.
- `frame-agent-connectors` comes from Frame Server's `v0.1.0` release commit `7bb94c48812713429bda58316d0299d4487709d8`.

The aliases preserve existing `@frame/*` imports. Frame's package build publishes under `@poziel/*`, while Frame Server uses Cargo Git dependencies. No crates.io publication is assumed.

GitHub npm packages require authentication. Create a classic GitHub token with `read:packages`, then sign in locally:

```sh
npm login --scope=@poziel --auth-type=legacy --registry=https://npm.pkg.github.com
bun run deps:released
```

Enter your GitHub username and use the token as the password. Credentials stay in your user npm configuration. `.npmrc` in this repository contains only the registry scope. CI supplies `NODE_AUTH_TOKEN` through its workflow environment. Do not paste credentials into source files or commit them.

After the migration is published and pinned, `bun run build:released` is intended to install the locked published packages without sibling repositories. GitHub release jobs require this mode; they reject the current temporary Cargo path.

## Local development

Use this layout:

```text
1-codes/
  pixen/
  frame/
  frame-server/
    packages/agents-rust/
```

Install Frame's own dependencies and build its library and Tauri package first:

```sh
cd ../frame
bun install --frozen-lockfile
bun run build:library
bun run --cwd packages/tauri build
cd ../pixen
bun run dev
```

`bun run dev`, `bun run build`, and `bun run build:debug` refresh local Frame packages and apply a Cargo path patch pointing to the sibling Frame Server package. Missing checkouts produce an error rather than silently switching to published code. Local mode does not require GitHub npm credentials. Cargo may still fetch the public Git dependency and other Rust dependencies when resolving its graph.

Useful commands:

| Command                                    | Dependency mode                                             |
| ------------------------------------------ | ----------------------------------------------------------- |
| `bun run deps:local`                       | Install local Frame outputs for frontend checks and tests   |
| `bun run dev`                              | Local Frame and Frame Server; start the desktop dev session |
| `bun run build:debug`                      | Local Frame and Frame Server; build the debug desktop       |
| `bun scripts/dependencies.ts native-check` | Check the desktop against local Frame Server                |
| `bun run deps:released`                    | Restore the published frontend dependency installation      |
| `bun run build` / `bun run release`        | Local sources; production build / Windows ZIP               |
| `bun run build:released`                   | Published dependency production build                       |

After changing Frame source, rebuild the affected Frame package, then run `bun run deps:local` again to refresh Bun's local package copies. The local development runner does this refresh when it starts. Frontend `check`/`test` commands use the currently installed mode.

For different checkout locations, set `PIXEN_FRAME_PATH` and `PIXEN_FRAME_SERVER_PATH` to absolute repository paths before running local commands. Frame Server's directory is separate from the old Frame Native path.

The installer briefly substitutes local frontend dependency paths, invokes `bun install --no-save`, and restores `package.json` even on an install failure. It does not save a local Bun lockfile. Native local commands pass Cargo's path patch through command arguments and restore the release desktop lockfile when the command exits, including normal interruption. Avoid running native builds or editing that lockfile concurrently. A forcibly killed process can bypass cleanup; inspect the Git diff before committing if that happens.

The raw `bun run tauri ...` and `cargo ...` commands bypass the mode wrappers. Use the named scripts when you want dependency-mode guarantees.
