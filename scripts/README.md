# Project scripts

Run scripts with Bun from the repository root. PowerShell and Python are not required. Rust, Bun, and the native desktop prerequisites are still needed for builds; Linux/macOS archives use the system `tar` command.

| Command or script | Purpose |
| --- | --- |
| `bun run dev`, `bun run build`, `bun run build:debug` | Use `dependencies.ts` to select local Frame dependencies and run the desktop workflow. |
| `bun run deps:local`, `bun run deps:released` | Select local or published frontend dependencies. See [dependency setup](../docs/development/dependencies.md) for the current migration constraint. |
| `bun run release` | Build the desktop and headless tools, then package the current host's executables. Windows produces a ZIP; Linux/macOS produce a tar.gz. |
| `bun scripts/release.ts --package-only` | Package already-built executables. Used by Windows release CI. |
| `bun scripts/package-tools.ts --target <rust-target>` | Package already-built CLI/MCP tools without the desktop. The target labels the native build; this does not cross-compile binaries. |
| `bun scripts/verify-windows-package.ts --archive <path.zip>` | Check that a Windows application ZIP contains the editor and both tools together. `--directory <folder>` selects exactly one ZIP. |
| `bun scripts/check-release-dependencies.ts` | Check published dependency configuration, lockfile consistency, and release versions in CI. |
| `bun scripts/prepare-release-config.ts` | Generate Tauri's platform packaging configuration after CI creates the icons. |

`packaging.ts` contains shared file-copy, checksum, archive, and command helpers. Packages include the complete `playbook/` tree with workflows, rules, agent descriptions, MIT license and source notices. Generated packages are written under `target/portable/` or `target/packages/`. Repeated application packaging creates a separate folder when one already exists, preserving existing portable profiles and running installations.

On Windows, run `bun scripts/smoke-guidance.ts --archive <path.zip>` against a built application or tools package. It extracts an isolated installation and checks resource, prompt and tool reads from its MCP executable running in an unrelated empty working directory. See [guidance discovery](../docs/mcp/guidance.md).

Run `bun run check:scripts` for TypeScript checks and `bun test tests/scripts` for packaging tests. `bun run test` includes both frontend and script tests. CI runs packaging checks on Windows, Linux, and macOS.

## Reading and maintaining the scripts

Each TypeScript file starts with a file overview describing its role and how it is invoked. Exported functions and their option/result interfaces have TSDoc for editor hovers. Workflow comments explain filesystem changes, cleanup, and publication boundaries; comments inside a workflow explain decisions rather than translating individual statements.

The entry points are `release.ts` for the full application, `package-tools.ts` for CLI/MCP archives, and `verify-windows-package.ts` for Windows ZIP checks. Each keeps its command-line invocation under `import.meta.main`, so packaging tests can import the functions without starting a build or creating an archive. Shared helpers live in `packaging.ts`. `dependencies.ts` dispatches development commands directly, while the two CI configuration/check scripts run their steps from top to bottom.

Use the [Google TypeScript Style Guide](https://google.github.io/styleguide/tsguide.html) as the baseline, with [Frame's TypeScript conventions](../../frame/docs/contributing/coding-standards/typescript.md) for TSDoc, interface spacing, and parameter layout. These executable workflows use plain functions and data interfaces; they do not need classes or static container objects. Keep names descriptive, use named exports and type-only imports, declare return types on shared functions, and document inputs, side effects, failure behavior, and cleanup where they matter. Keep function signatures on one line when they fit 80 columns and have at most six parameters; otherwise place parameters vertically. Markdown prose remains on one physical line per paragraph.

When changing packaging, preserve the separation between fresh staging trees and existing installations. The archive contains only shipped files, and a repeated application package creates a new installation folder. Dependency setup must restore temporary manifest/lockfile changes even when a child command fails. The release-dependency prerequisite runs before installation, so it must not gain runtime imports that require installed npm packages.
