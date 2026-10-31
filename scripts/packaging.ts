/**
 * @fileoverview Shared procedural helpers for release and tools packaging.
 *
 * Callers stage only shipped files before creating an archive. These helpers do
 * not read portable profiles or launch a shell to interpret command arguments.
 */

import {createHash} from 'node:crypto';
import {
  chmodSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import {basename, dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {zipSync} from 'fflate';

/** Repository location derived from this module, independent of the caller. */
const PROJECT_ROOT = fileURLToPath(new URL('../', import.meta.url));

/** Fields read or temporarily overridden in the repository's package.json. */
export interface PackageManifest {

  /** Application version used in archive names and release checks. */
  readonly version: string;

  /** Runtime package specifiers, including the Frame package aliases. */
  dependencies: Record<string, string>;

  /** Development package specifiers checked for local release overrides. */
  readonly devDependencies?: Record<string, string>;

  /** Resolution overrides temporarily redirected during local development. */
  overrides?: Record<string, string>;
}

/**
 * Reads the repository-owned manifest without discarding its additional fields.
 *
 * @param projectRoot - Checkout containing the package.json to read.
 * @remarks The repository's manifest is trusted configuration, not user input.
 */
export function readPackageManifest(projectRoot: string): PackageManifest {
  const manifestPath = join(projectRoot, 'package.json');
  return JSON.parse(readFileSync(manifestPath, 'utf8')) as PackageManifest;
}

/**
 * Rejects target labels that could introduce path separators into archive names.
 *
 * @returns The unchanged Rust target label; this does not select a toolchain.
 * @throws If the label contains characters outside letters, digits, _ and -.
 */
export function validateTarget(target: string): string {
  if (!/^[a-zA-Z0-9_-]+$/.test(target)) {
    throw new Error('Invalid target name.');
  }
  return target;
}

/**
 * Runs an argument array directly, forwarding terminal input and diagnostics.
 *
 * @param command - Executable followed by its individual arguments.
 * @param workingDirectory - Directory in which the child process starts.
 * @throws If the executable cannot start or exits with a nonzero status.
 */
export async function runCommand(
  command: string[],
  workingDirectory = PROJECT_ROOT
): Promise<void> {
  const child = Bun.spawn(command, {
    cwd: workingDirectory,
    stdin: 'inherit',
    stdout: 'inherit',
    stderr: 'inherit',
  });
  const exitCode = await child.exited;
  if (exitCode !== 0) {
    throw new Error(`Command failed (${exitCode}): ${command.join(' ')}`);
  }
}

/**
 * Copies a required shipped file or directory, creating its destination parent.
 *
 * @throws If the source is missing or the filesystem copy fails.
 */
export function copyRequired(source: string, destination: string): void {
  if (!existsSync(source)) {
    throw new Error(`Required package file missing: ${source}`);
  }
  mkdirSync(dirname(destination), {recursive: true});
  cpSync(source, destination, {recursive: true});
}

/**
 * Copies nonempty release executables and writes their SHA-256 checksum file.
 *
 * @param projectRoot - Checkout containing target/release.
 * @param destination - Fresh package staging directory.
 * @param target - Rust target label used to select the Windows .exe suffix.
 * @param includeDesktop - Whether to include the editor with the CLI and MCP.
 * @remarks Non-Windows executables receive executable permissions on POSIX.
 */
export function copyReleaseBinaries(
  projectRoot: string,
  destination: string,
  target: string,
  includeDesktop: boolean
): void {
  const executableSuffix = target.includes('windows') ? '.exe' : '';
  const binaryNames = includeDesktop ?
    ['pixen', 'pixen-cli', 'pixen-mcp'] : ['pixen-cli', 'pixen-mcp'];
  const filenames = binaryNames.map(name => name + executableSuffix);

  for (const filename of filenames) {
    const sourcePath = join(projectRoot, 'target/release', filename);
    if (!existsSync(sourcePath) || !statSync(sourcePath).isFile() ||
        statSync(sourcePath).size === 0) {
      throw new Error(`Release executable missing or empty: ${sourcePath}`);
    }
    copyRequired(sourcePath, join(destination, filename));
    if (!executableSuffix) {
      chmodSync(join(destination, filename), 0o755);
    }
  }

  // Hash the staged bytes, so the checksum describes exactly what is shipped.
  const checksums = filenames.map(filename => {
    const contents = readFileSync(join(destination, filename));
    const hash = createHash('sha256').update(contents).digest('hex');
    return `${hash}  ${filename}`;
  });
  writeFileSync(join(destination, 'SHA256SUMS.txt'), checksums.join('\n') + '\n');
}

/**
 * Copies the complete playbook, including imported workflows and source notices.
 *
 * @param projectRoot - Checkout containing playbook/.
 * @param destination - Fresh package staging directory.
 * @throws If required entry points or attribution are absent.
 */
export function copyPlaybook(
  projectRoot: string,
  destination: string
): void {
  const sourcePath = join(projectRoot, 'playbook');
  for (const required of [
    'skills/pixen/SKILL.md', 'skills/pixel-art/SKILL.md',
    'skills/draw/SKILL.md', 'rules/00-core-principles.md',
    'agents/pixel-critic.md', 'LICENSE', 'NOTICE.md',
  ]) {
    if (!existsSync(join(sourcePath, required))) {
      throw new Error(`Missing playbook file: ${required}`);
    }
  }
  for (const removed of ['submit', 'gallery']) {
    if (existsSync(join(sourcePath, 'skills', removed))) {
      throw new Error(`Removed workflow must not be packaged: ${removed}`);
    }
  }
  copyRequired(sourcePath, join(destination, 'playbook'));
}

/**
 * Collects a staging tree into ZIP entries without following symbolic links.
 *
 * @param directory - Directory whose immediate entries should be inspected.
 * @param prefix - Forward-slash archive path, including the package root name.
 * @param files - Accumulator of file bytes indexed by archive entry path.
 */
function collectZipEntries(
  directory: string,
  prefix: string,
  files: Record<string, Uint8Array>
): void {
  for (const entry of readdirSync(directory, {withFileTypes: true})) {
    const entryPath = join(directory, entry.name);
    const archivePath = `${prefix}/${entry.name}`;
    if (entry.isDirectory()) {
      collectZipEntries(entryPath, archivePath, files);
    } else if (entry.isFile()) {
      files[archivePath] = readFileSync(entryPath);
    } else {
      throw new Error(`Unsupported package entry: ${entryPath}`);
    }
  }
}

/**
 * Creates a ZIP or tar.gz with one package root, then installs the completed file.
 *
 * @param directory - Complete staging tree to archive.
 * @param archivePath - Final archive filename, replaced after creation succeeds.
 * @param useZip - Use ZIP for Windows; otherwise use system tar for POSIX modes.
 * @remarks Only this invocation's temporary archive is removed on failure.
 */
export async function createPackageArchive(
  directory: string,
  archivePath: string,
  useZip: boolean
): Promise<void> {
  const temporaryPath = `${archivePath}.${crypto.randomUUID()}.tmp`;
  try {
    if (useZip) {
      const files: Record<string, Uint8Array> = {};
      collectZipEntries(directory, basename(directory), files);
      writeFileSync(temporaryPath, zipSync(files, {level: 6}));
    } else {
      // System tar preserves the executable modes needed by Linux and macOS.
      await runCommand([
        'tar', '-czf', temporaryPath, '-C', dirname(directory),
        basename(directory),
      ]);
    }
    renameSync(temporaryPath, archivePath);
  } finally {
    if (existsSync(temporaryPath)) {
      unlinkSync(temporaryPath);
    }
  }
}

/**
 * Reads the value following a required command-line option.
 *
 * @param args - Arguments after the script filename.
 * @param name - Full option spelling, such as --target or --archive.
 * @throws If the option is absent or followed by another option instead of data.
 */
export function readRequiredOption(args: string[], name: string): string {
  const optionIndex = args.indexOf(name);
  const value = optionIndex < 0 ? undefined : args[optionIndex + 1];
  if (!value || value.startsWith('--')) {
    throw new Error(`Missing ${name} value.`);
  }
  return value;
}

/** Resolves a checkout override, or defaults to this script's own checkout. */
export function resolveProjectRoot(projectRoot = PROJECT_ROOT): string {
  return resolve(projectRoot);
}
