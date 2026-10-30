/**
 * @fileoverview Checks that an application ZIP contains all three executables.
 *
 * Run with `--archive <file.zip>`, or `--directory <folder>` to require exactly
 * one ZIP in a CI output directory. Verification reads ZIP entries in memory;
 * it does not extract files or execute the packaged binaries.
 */

import {readFileSync, readdirSync} from 'node:fs';
import {join} from 'node:path';
import {unzipSync} from 'fflate';
import {readRequiredOption} from './packaging';

/**
 * Requires one nonempty editor, CLI, and MCP executable in the same ZIP folder.
 *
 * @param archivePath - Application ZIP to inspect.
 * @throws If the ZIP cannot be decoded or its executable layout is invalid.
 * @remarks This checks package layout, not executable signatures or runtime use.
 */
export function verifyWindowsPackage(archivePath: string): void {
  const files = unzipSync(readFileSync(archivePath));
  let executableDirectory: string | undefined;

  for (const binaryName of ['pixen.exe', 'pixen-cli.exe', 'pixen-mcp.exe']) {
    const matchingEntries = Object.keys(files).filter(entryName => {
      return entryName.split('/').at(-1) === binaryName &&
        files[entryName].length > 0;
    });
    if (matchingEntries.length !== 1) {
      throw new Error(
        `Application ZIP must contain exactly one nonempty ${binaryName}`
      );
    }

    // Preview launching depends on the tools being beside the editor.
    const parentDirectory = matchingEntries[0].slice(0, -binaryName.length);
    if (executableDirectory !== undefined &&
        executableDirectory !== parentDirectory) {
      throw new Error(
        `${binaryName} must be beside pixen.exe in the application ZIP`
      );
    }
    executableDirectory = parentDirectory;
  }
}

/** Selects an explicit archive or the sole ZIP in a generated output folder. */
function resolveArchivePath(args: string[]): string {
  if (!args.includes('--directory')) {
    return readRequiredOption(args, '--archive');
  }

  const directory = readRequiredOption(args, '--directory');
  const filenames = readdirSync(directory).filter(name => name.endsWith('.zip'));
  if (filenames.length !== 1) {
    throw new Error('Expected exactly one Windows application ZIP.');
  }
  return join(directory, filenames[0]);
}

// Command-line entry point. Tests import the verifier without running it.
if (import.meta.main) {
  const archivePath = resolveArchivePath(process.argv.slice(2));
  verifyWindowsPackage(archivePath);
  console.log(
    `Verified application ZIP: ${archivePath} (editor, CLI, and MCP server)`
  );
}
