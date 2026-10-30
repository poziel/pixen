/**
 * @fileoverview Validates published dependency configuration and release versions.
 *
 * Run from the repository root before release builds. Local dependency overrides
 * deliberately fail this check; development builds use dependencies.ts instead.
 */

import {readFileSync} from 'node:fs';
import type {PackageManifest} from './packaging';

/** Dependency pins recorded for a workspace in Bun's repository-owned lockfile. */
interface LockedWorkspace {

  /** Package names mapped to the specifiers committed in package.json. */
  readonly dependencies: Readonly<Record<string, string>>;
}

/** Lockfile fields used to compare the root workspace's published Frame pins. */
interface FrontendLockfile {

  /** Workspaces keyed by relative path; the empty key identifies the root. */
  readonly workspaces: Readonly<Record<string, LockedWorkspace>>;
}

/** Application version field read from the repository's Tauri configuration. */
interface DesktopVersionConfig {

  /** Must match the frontend and both Cargo package versions. */
  readonly version: string;
}

// Reject machine-specific frontend sources and native sibling path overrides.
// This prerequisite runs before bun install, so keep runtime imports built-in.
const manifest = JSON.parse(
  readFileSync('package.json', 'utf8')
) as PackageManifest;
const localDependencies = Object.entries({
  ...manifest.dependencies,
  ...manifest.devDependencies,
  ...manifest.overrides,
}).filter(([, value]) => /^(file:|link:|workspace:)/.test(String(value)));
const nativeManifest = readFileSync('src-tauri/Cargo.toml', 'utf8');
const connectorDependency =
  nativeManifest.match(/^frame-agent-connectors\s*=.*$/m)?.[0];
if (localDependencies.length > 0 || !connectorDependency ||
    /\bpath\s*=/.test(connectorDependency)) {
  console.error(
    'Desktop releases require published Frame and Frame Server dependencies.'
  );
  for (const [name] of localDependencies) {
    console.error(`Replace the local dependency: ${name}`);
  }
  if (!connectorDependency || /\bpath\s*=/.test(connectorDependency)) {
    console.error(
      'Replace frame-agent-connectors with a pinned Cargo registry or ' +
        'Git dependency.'
    );
  }
  console.error(
    'Update and commit bun.lock and src-tauri/Cargo.lock. ' +
      'See docs/development/releases.md.'
  );
  process.exit(1);
}

// Lockfile and JSON assertions describe trusted repository configuration.
const lockfile = Bun.JSON5.parse(
  readFileSync('bun.lock', 'utf8')
) as FrontendLockfile;
for (const name of ['@frame/core', '@frame/tauri']) {
  if (lockfile.workspaces[''].dependencies[name] !==
      manifest.dependencies[name]) {
    throw new Error(
      `bun.lock is stale for ${name}; authenticate to GitHub Packages ` +
        'and run bun install.'
    );
  }
}

// Installers, binary metadata, and release tags must agree on the same version.
const version = manifest.version;
const desktopConfig = JSON.parse(
  readFileSync('src-tauri/tauri.conf.json', 'utf8')
) as DesktopVersionConfig;
for (const manifestPath of ['Cargo.toml', 'src-tauri/Cargo.toml']) {
  const cargoVersion = readFileSync(manifestPath, 'utf8')
    .match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (cargoVersion !== version) {
    throw new Error(`${manifestPath} version must match package.json`);
  }
}
if (desktopConfig.version !== version) {
  throw new Error('Tauri version must match package.json');
}
if (process.env.GITHUB_REF_TYPE === 'tag' &&
    process.env.GITHUB_REF_NAME !== `v${version}`) {
  throw new Error('Release tag must equal v<package.json version>');
}
console.log(`Release dependencies and version ${version} are configured.`);
