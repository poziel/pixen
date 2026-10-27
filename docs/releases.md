# GitHub builds and releases

## Available workflows

- **Build tools** runs on pushes and pull requests to `main`, or manually. It
  tests the Rust engine and builds CLI/MCP archives for Windows x64, Linux x64,
  macOS Apple Silicon, and macOS Intel. Download these from a run's Artifacts.
  They do not contain the desktop editor.
- **Desktop release** runs manually for validation or when a `v*` tag is pushed.
  It checks dependencies first, then tests and builds the editor on the same
  four platforms. Manual runs retain artifacts without publishing a release.
  Tags publish a GitHub prerelease only after every platform succeeds.

Desktop output includes a Windows NSIS installer and a ZIP with the editor,
CLI/MCP tools, themes, and skills; macOS DMGs; Linux AppImage and Debian packages;
and separate headless tools archives. Archive checksums accompany tagged releases.

These are preview distribution targets, not verified platform support. Windows
ZIP packaging preserves the existing sibling-executable preview workflow.
macOS/Linux desktop preview integration from the separate CLI/MCP packages still
needs validation and may require platform-aware installation/launch handling.
Screen color capture is Windows-only. Windows packages are unsigned; macOS
packages are ad-hoc signed and not notarized. Configure production signing
before promoting packages for general distribution.

## Required dependency publication

Desktop builds are deliberately blocked while Pixen references local Frame
folders. The prerequisites job explains what remains to configure; it does not
substitute an older public Frame commit for the unpublished local work.

Once Frame packages are ready:

1. Replace `@frame/core` and `@frame/tauri` local dependencies and the core override
   in `package.json` with the published packages at pinned versions. npm aliases
   can preserve the current import names when publishing under an owner scope.
2. Replace the `frame-agent-connectors` path in `src-tauri/Cargo.toml` with a
   registry dependency or a Git dependency pinned to a full commit. GitHub's npm
   registry distributes JavaScript packages; Cargo needs its own source.
3. Regenerate and commit `bun.lock` and `src-tauri/Cargo.lock`. Check that a clean
   checkout builds without the sibling Frame folders.
4. For GitHub npm packages, set repository Actions variable `FRAME_NPM_SCOPE`
   to the published scope, such as `@poziel`. Grant Pixen Actions read access to
   the packages. The workflow uses its `GITHUB_TOKEN`; if cross-repository access
   requires another credential, set secret `FRAME_PACKAGES_TOKEN` with read access.
   For public npm packages, leave the scope variable unset.
5. If Frame Native uses a private Git repository or private Cargo registry,
   configure the corresponding Cargo authentication before running desktop jobs.
   The current workflow does not provision that authentication.
6. Update `scripts/release.ps1` to install published packages without rebuilding
   a sibling Frame checkout. That source-mode step remains in local builds;
   GitHub uses the script's `-PackageOnly` option after building.

Never commit registry credentials. Run `bun scripts/check-release-dependencies.ts`
to check dependency prerequisites locally. A successful preflight checks manifest
configuration; actual install/build jobs verify package availability and compatibility.

## Publish a version

First run **Desktop release** manually on `main` and inspect the artifacts.
Keep `package.json`, both Cargo manifests, and `src-tauri/tauri.conf.json` versions
aligned. Commit the corresponding lockfile changes, then tag that commit:

```sh
git tag v0.2.0
git push origin v0.2.0
```

Choose a new unused version for each release. The workflow verifies tag/version
agreement and does not overwrite an existing release. Do not tag while the
dependency-publication prerequisite is pending.

See [Tauri's GitHub pipeline guide](https://tauri.app/distribute/pipelines/github/)
and [signing documentation](https://tauri.app/distribute/) for subsequent signing
and distribution setup. Automatic application updates are not configured.
