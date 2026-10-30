/**
 * @fileoverview Packages already-built CLI and MCP binaries without the editor.
 *
 * Run with `bun scripts/package-tools.ts --target <rust-target>`. The target
 * labels existing binaries; this script does not build or cross-compile them.
 */

import {mkdirSync, mkdtempSync, rmSync, writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {
  copyBundledSkills,
  copyReleaseBinaries,
  copyRequired,
  createPackageArchive,
  readPackageManifest,
  readRequiredOption,
  resolveProjectRoot,
  validateTarget,
} from './packaging';

/** Inputs for creating a headless tools distribution. */
export interface ToolsPackageOptions {

  /** Optional checkout override used by isolated packaging tests. */
  readonly root?: string;

  /** Rust target label matching the binaries already in target/release. */
  readonly target: string;
}

/**
 * Stages both tools, their skills, license, and checksums before archiving them.
 *
 * @returns Absolute path to the ZIP or tar.gz under target/packages.
 * @throws If a required binary or skill is missing, or archive creation fails.
 * @remarks The unique staging directory is removed on success and failure.
 */
export async function packageTools(
  options: ToolsPackageOptions
): Promise<string> {
  const projectRoot = resolveProjectRoot(options.root);
  const target = validateTarget(options.target);
  const manifest = readPackageManifest(projectRoot);
  const packageName = `Pixen-tools-${manifest.version}-${target}`;

  // Stage only shipped files; never reuse a previous package's directory.
  const stagingParent = join(projectRoot, 'target/package-staging');
  mkdirSync(stagingParent, {recursive: true});
  const stagingDirectory = mkdtempSync(join(stagingParent, 'tools-'));
  const packageDirectory = join(stagingDirectory, packageName);
  try {
    mkdirSync(packageDirectory);
    copyReleaseBinaries(projectRoot, packageDirectory, target, false);
    copyRequired(
      join(projectRoot, 'LICENSE'), join(packageDirectory, 'LICENSE')
    );
    copyBundledSkills(projectRoot, packageDirectory);
    writeFileSync(join(packageDirectory, 'README.txt'), `Pixen headless tools

pixen-cli: file commands and exports; use --help for commands.
pixen-mcp: stdio MCP server, launched by your MCP client.
This package does not contain the desktop editor. Install the matching pixen executable beside these tools before requesting desktop previews. PNG inspection and headless editing remain available.
Video export requires FFmpeg. AI skills are included in skills/.
`);

    // Install the archive only after the complete staged tree is available.
    const outputDirectory = join(projectRoot, 'target/packages');
    mkdirSync(outputDirectory, {recursive: true});
    const useZip = target.includes('windows');
    const archiveExtension = useZip ? 'zip' : 'tar.gz';
    const archivePath = join(
      outputDirectory, `${packageName}.${archiveExtension}`
    );
    await createPackageArchive(packageDirectory, archivePath, useZip);
    return archivePath;
  } finally {
    // Remove only the unique child returned by mkdtemp above.
    rmSync(stagingDirectory, {recursive: true, force: true});
  }
}

// Command-line entry point. Imports from tests do not create a package.
if (import.meta.main) {
  const target = readRequiredOption(process.argv.slice(2), '--target');
  console.log(await packageTools({target}));
}
