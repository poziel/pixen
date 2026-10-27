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

## Dependency and package access setup

Published dependencies are pinned in the manifests and lockfiles. See
[dependency modes](dependencies.md) for the exact Frame package versions,
Frame Server release commit, and the local development workflow.

In each GitHub package's **Package settings > Manage Actions access**, add the
`poziel/pixen` repository with **Read** access. Do this for `frame-core` and
`frame-tauri`. The desktop workflow supplies its built-in `GITHUB_TOKEN` with
`packages: read`. No registry token belongs in source control.

If your package settings require a separate token instead, repository secret
`FRAME_PACKAGES_TOKEN` can supply a classic token with `read:packages` access.
Frame Server's pinned Git source is public and does not require package-registry
authentication. No `FRAME_NPM_SCOPE` variable is needed: `.npmrc` configures the
actual `@poziel` scope.

The prerequisites job checks manifests and versions; package access is verified
by the install step. Release jobs always use published dependencies and never
invoke local development overrides. Manual runs test the full package build
without publishing a release.

## Publish a version

First run **Desktop release** manually on `main` and inspect the artifacts.
Keep `package.json`, both Cargo manifests, and `src-tauri/tauri.conf.json` versions
aligned. Commit the corresponding lockfile changes, then tag that commit:

```sh
git tag v0.2.0
git push origin v0.2.0
```

Choose a new unused version for each release. The workflow verifies tag/version
agreement and does not overwrite an existing release. Verify package access and all platform builds before tagging.

See [Tauri's GitHub pipeline guide](https://tauri.app/distribute/pipelines/github/)
and [signing documentation](https://tauri.app/distribute/) for subsequent signing
and distribution setup. Automatic application updates are not configured.
