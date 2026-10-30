/**
 * @fileoverview Builds and packages the editor, CLI, MCP server, and resources.
 *
 * `bun run release` builds from local dependencies. Release CI passes
 * `--package-only` after building with its published dependency configuration.
 * Each package is staged separately from existing installations and user data.
 */

import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import {join} from 'node:path';
import {
  copyBundledSkills,
  copyReleaseBinaries,
  copyRequired,
  createPackageArchive,
  readPackageManifest,
  resolveProjectRoot,
  runCommand,
  validateTarget,
} from './packaging';
import {verifyWindowsPackage} from './verify-windows-package';

/** Build and package settings for one native application distribution. */
export interface ReleasePackageOptions {

  /** Optional checkout override used by isolated packaging tests. */
  readonly root?: string;

  /** Rust target label; defaults to the host reported by rustc. */
  readonly target?: string;

  /** Skip builds and package the executables already in target/release. */
  readonly packageOnly?: boolean;
}

/** Published paths returned after staging, archiving, and copying succeed. */
export interface ReleasePackageResult {

  /** Completed ZIP or tar.gz under target/portable. */
  readonly archive: string;

  /** Extracted application folder, distinct from any existing installation. */
  readonly directory: string;
}

/**
 * Builds when requested, then stages and publishes one native application.
 *
 * @returns The completed archive and extracted application directory.
 * @throws If building, staging, archiving, verification, or publication fails.
 * @remarks Existing application folders are never overwritten. Staging is
 * removed on success and failure; published output is retained.
 */
export async function packageRelease(
  options: ReleasePackageOptions = {}
): Promise<ReleasePackageResult> {
  const projectRoot = resolveProjectRoot(options.root);
  const target = validateTarget(options.target || readHostTarget(projectRoot));
  if (!options.packageOnly) {
    await runCommand([process.execPath, 'run', 'build'], projectRoot);
    await runCommand([process.execPath, 'run', 'build:tools'], projectRoot);
  }

  const manifest = readPackageManifest(projectRoot);
  const packageName = `Pixen-${manifest.version}-${target}`;
  const useZip = target.includes('windows');
  const outputDirectory = join(projectRoot, 'target/portable');
  mkdirSync(outputDirectory, {recursive: true});

  // A fresh staging tree prevents portable profiles entering the distribution.
  const stagingDirectory = mkdtempSync(join(outputDirectory, '.staging-'));
  const packageDirectory = join(stagingDirectory, packageName);
  try {
    mkdirSync(packageDirectory);
    copyReleaseBinaries(projectRoot, packageDirectory, target, true);
    copyRequired(
      join(projectRoot, 'LICENSE'), join(packageDirectory, 'LICENSE')
    );
    copyBundledSkills(projectRoot, packageDirectory);
    writeThemeFiles(projectRoot, packageDirectory);
    writeReleaseReadme(packageDirectory, manifest.version, target, useZip);

    // Validate the completed Windows archive before publishing its folder.
    const archiveExtension = useZip ? 'zip' : 'tar.gz';
    const archivePath = join(
      outputDirectory, `${packageName}.${archiveExtension}`
    );
    await createPackageArchive(packageDirectory, archivePath, useZip);
    if (useZip) {
      verifyWindowsPackage(archivePath);
    }

    // Never replace a running executable or an existing portable installation.
    let publishedDirectory = join(outputDirectory, packageName);
    if (existsSync(publishedDirectory)) {
      const buildSuffix = `${Date.now()}-${crypto.randomUUID().slice(0, 8)}`;
      publishedDirectory += `-build-${buildSuffix}`;
    }
    cpSync(packageDirectory, publishedDirectory, {recursive: true});
    return {archive: archivePath, directory: publishedDirectory};
  } finally {
    // Remove only the unique staging child created by this invocation.
    rmSync(stagingDirectory, {recursive: true, force: true});
  }
}

/** Reads Rust's native host target without compiling or selecting a toolchain. */
function readHostTarget(projectRoot: string): string {
  const result = Bun.spawnSync(['rustc', '-vV'], {cwd: projectRoot});
  if (result.exitCode !== 0) {
    throw new Error('Could not determine the Rust host target.');
  }
  const target = result.stdout.toString().match(/^host:\s*(\S+)/m)?.[1];
  if (!target) {
    throw new Error('Rust did not report a host target.');
  }
  return target;
}

/** Creates theme instructions and a template with its own selectable identity. */
function writeThemeFiles(projectRoot: string, packageDirectory: string): void {
  mkdirSync(join(packageDirectory, 'themes'));
  writeFileSync(
    join(packageDirectory, 'themes/README.txt'),
    'Put custom Frame themes here, restart Pixen, and select one in Settings > ' +
      'Appearance. Copy an editable template from ../theme-examples/.\n'
  );
  mkdirSync(join(packageDirectory, 'theme-examples'));
  const themeSource = readFileSync(
    join(projectRoot, 'assets/themes/pixen.yaml'), 'utf8'
  );
  const themeExample = themeSource.replace('id: pixen', 'id: pixen-example')
    .replace('name: Pixen', 'name: Pixen example');
  writeFileSync(
    join(packageDirectory, 'theme-examples/pixen-example.yaml'), themeExample
  );
}

/** Writes platform-specific launch instructions and portable-update guidance. */
function writeReleaseReadme(
  packageDirectory: string,
  version: string,
  target: string,
  useZip: boolean
): void {
  writeFileSync(join(packageDirectory, 'README.txt'), `Pixen ${version} — ${target}

Run ${useZip ? 'pixen.exe' : './pixen'} to open the editor. Keep the desktop, CLI, and MCP executables together for live previews.
${useZip ? 'Microsoft Edge WebView2 Runtime must be installed.\n' : 'Desktop dependencies for your platform must be installed. Prefer the platform installer for application integration.\n'}
Storage modes are available in Settings > Installation:
- Default: use this computer's user folders (no portable marker).
- Portable: keep Pixen data in data/ beside the executables (empty portable marker).
- Semi-portable: keep configuration, themes, and recovery in data/; browser/cache/temp/logs remain on this computer (portable marker containing semi-portable).

Apply and restart transfers preferences and unfinished artwork, backing up destination conflicts. Original files are retained; undo history and AI conversations reset. Close other Pixen windows and MCP connections first. Manually changing the marker selects storage without transferring data. Keep data/, portable, custom themes, and backups when updating.

Keyboard shortcuts use keybindings.json beside settings.json. AI providers require separately installed and authenticated tools. Install the bundled skills/pixen and skills/pixel-art through your AI client; bundling does not register them globally. Video export requires FFmpeg.

Configure your MCP client to launch the absolute path to ${useZip ? 'pixen-mcp.exe' : 'pixen-mcp'}, optionally with --viewer. Do not start a second server manually for the same connection.
`);
}

// Command-line entry point. Tests import packageRelease without building.
if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args.some(arg => arg !== '--package-only')) {
    throw new Error('Usage: bun scripts/release.ts [--package-only]');
  }
  const result = await packageRelease({
    packageOnly: args.includes('--package-only'),
  });
  console.log(result);
}
