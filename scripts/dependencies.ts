/**
 * @fileoverview Selects local or published Frame dependencies for Bun commands.
 *
 * Local setup temporarily rewrites package.json; native commands temporarily
 * resolve a Cargo path patch. Both workflows restore committed configuration in
 * finally blocks. The installed node_modules remain in the selected mode.
 */

import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import {dirname, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import type {PackageManifest} from './packaging';

/** Dependency source recorded for diagnostics, not an automatic build selector. */
type DependencyMode = 'local' | 'released';

// Repository and sibling locations. Environment overrides support other layouts.
const PROJECT_ROOT = fileURLToPath(new URL('../', import.meta.url));
const FRAME_ROOT = resolve(
  process.env.PIXEN_FRAME_PATH ?? resolve(PROJECT_ROOT, '../frame')
);
const FRAME_SERVER_ROOT = resolve(
  process.env.PIXEN_FRAME_SERVER_PATH ?? resolve(PROJECT_ROOT, '../frame-server')
);
const DEPENDENCY_STATE_PATH = resolve(
  PROJECT_ROOT, 'target/dependency-mode.json'
);
const BUN_EXECUTABLE = process.execPath;

// Track the native child so normal interruption can reach its cleanup boundary.
let activeChild: ReturnType<typeof Bun.spawn> | undefined;
for (const signal of ['SIGINT', 'SIGTERM'] as const) {
  process.on(signal, () => activeChild?.kill(signal));
}

/**
 * Runs a dependency/build command while tracking it for signal forwarding.
 *
 * @param args - Executable and its separate arguments; no shell is involved.
 * @param workingDirectory - Checkout in which the process should run.
 * @throws If the child fails; callers' finally blocks still restore their files.
 */
async function runDependencyCommand(
  args: string[],
  workingDirectory = PROJECT_ROOT
): Promise<void> {
  const child = Bun.spawn(args, {
    cwd: workingDirectory,
    stdin: 'inherit',
    stdout: 'inherit',
    stderr: 'inherit',
  });
  activeChild = child;
  const exitCode = await child.exited;
  activeChild = undefined;
  if (exitCode !== 0) {
    throw new Error(
      `Command failed (${exitCode}): ${args.slice(0, 3).join(' ')}`
    );
  }
}

/** Records the installed mode and sibling paths under ignored build output. */
function recordDependencyMode(mode: DependencyMode): void {
  mkdirSync(dirname(DEPENDENCY_STATE_PATH), {recursive: true});
  const state = {mode, frame: FRAME_ROOT, server: FRAME_SERVER_ROOT};
  writeFileSync(DEPENDENCY_STATE_PATH, JSON.stringify(state, null, 2) + '\n');
}

/**
 * Installs sibling Frame outputs without saving local paths to the manifest.
 *
 * @remarks Missing frontend outputs are built first. Existing outputs must be
 * rebuilt explicitly after changing Frame source. Concurrent package.json edits
 * are preserved and reported rather than overwritten by restoration.
 */
async function installLocalDependencies(): Promise<void> {
  const requiredPaths = [
    resolve(FRAME_ROOT, 'packages/core/package.json'),
    resolve(FRAME_ROOT, 'packages/tauri/package.json'),
    resolve(FRAME_SERVER_ROOT, 'packages/agents-rust/Cargo.toml'),
  ];
  for (const path of requiredPaths) {
    if (!existsSync(path)) {
      throw new Error(
        `Local dependency missing: ${path}. ` +
          'See docs/development/dependencies.md.'
      );
    }
  }

  // Bun copies local package outputs; build missing entry points before install.
  const frontendBuilds = [
    ['packages/core/dist/frame.js', ['run', 'build:library']],
    ['packages/tauri/dist/index.js', ['run', '--cwd', 'packages/tauri', 'build']],
  ] as const;
  for (const [entry, args] of frontendBuilds) {
    if (!existsSync(resolve(FRAME_ROOT, entry))) {
      await runDependencyCommand([BUN_EXECUTABLE, ...args], FRAME_ROOT);
    }
  }

  const manifestPath = resolve(PROJECT_ROOT, 'package.json');
  const originalContents = readFileSync(manifestPath, 'utf8');
  const manifest = JSON.parse(originalContents) as PackageManifest;
  manifest.dependencies['@frame/core'] =
    `file:${resolve(FRAME_ROOT, 'packages/core').replaceAll('\\', '/')}`;
  manifest.dependencies['@frame/tauri'] =
    `file:${resolve(FRAME_ROOT, 'packages/tauri').replaceAll('\\', '/')}`;
  manifest.overrides = {
    ...manifest.overrides,
    '@frame/core': manifest.dependencies['@frame/core'],
  };
  const temporaryContents = JSON.stringify(manifest, null, 2) + '\n';

  // Restore only our temporary content; do not discard another editor's changes.
  writeFileSync(manifestPath, temporaryContents);
  try {
    await runDependencyCommand([BUN_EXECUTABLE, 'install', '--no-save']);
  } finally {
    if (readFileSync(manifestPath, 'utf8') === temporaryContents) {
      writeFileSync(manifestPath, originalContents);
    } else {
      throw new Error(
        'package.json changed during local setup; preserving that edit. ' +
          'Restore release dependencies before committing.'
      );
    }
  }

  recordDependencyMode('local');
  console.log(
    `Local Frame: ${FRAME_ROOT}\nLocal Frame Server: ${FRAME_SERVER_ROOT}`
  );
}

/** Installs exactly the published frontend dependencies in the committed lock. */
async function installReleasedDependencies(): Promise<void> {
  await runDependencyCommand([
    BUN_EXECUTABLE, 'install', '--frozen-lockfile',
  ]);
  recordDependencyMode('released');
}

/**
 * Runs a native development command with sibling Frame Server sources.
 *
 * @param command - Local dev/build mode or the native-check operation.
 * @param args - Additional arguments forwarded to Tauri or Cargo.
 * @remarks The desktop lockfile is restored after normal completion or failure.
 * Forced process termination can bypass cleanup; do not edit/build it in parallel.
 */
async function runLocalCommand(command: string, args: string[]): Promise<void> {
  if (process.env.CI) {
    throw new Error(
      'Local dependencies are disabled in CI. Use released builds.'
    );
  }
  await installLocalDependencies();

  const lockfilePath = resolve(PROJECT_ROOT, 'src-tauri/Cargo.lock');
  const originalContents = existsSync(lockfilePath) ?
    readFileSync(lockfilePath) : undefined;
  const connectorPath = resolve(FRAME_SERVER_ROOT, 'packages/agents-rust')
    .replaceAll('\\', '/');
  const cargoPatch =
    'patch."https://github.com/poziel/frame-server.git".' +
    `frame-agent-connectors.path=${JSON.stringify(connectorPath)}`;
  try {
    if (command === 'native-check') {
      await runDependencyCommand([
        'cargo', 'check', '--manifest-path', 'src-tauri/Cargo.toml',
        ...args, '--config', cargoPatch,
      ]);
    } else {
      const tauriArgs = command === 'dev' ? ['dev'] :
        command === 'debug' ? ['build', '--debug', '--no-bundle'] :
        ['build', '--no-bundle'];
      await runDependencyCommand([
        BUN_EXECUTABLE, 'run', 'tauri', ...tauriArgs, ...args,
        '--', '--config', cargoPatch,
      ]);
    }
  } finally {
    // Cargo records the patch resolution; keep it out of the release lockfile.
    if (originalContents) {
      writeFileSync(lockfilePath, originalContents);
    } else {
      rmSync(lockfilePath, {force: true});
    }
  }
}

// Command-line dispatch. Package scripts supply the mode; extra args pass through.
const [command, ...args] = process.argv.slice(2);
switch (command) {
  case 'local':
    await installLocalDependencies();
    break;
  case 'released':
    await installReleasedDependencies();
    break;
  case 'dev':
  case 'debug':
  case 'native-check':
  case 'build':
    await runLocalCommand(command, args);
    break;
  case 'released-build':
    await installReleasedDependencies();
    await runDependencyCommand([
      BUN_EXECUTABLE, 'run', 'tauri', 'build', '--no-bundle',
      ...args, '--', '--locked',
    ]);
    break;
  default:
    throw new Error(
      'Usage: bun scripts/dependencies.ts ' +
        'local|released|dev|debug|native-check|build|released-build'
    );
}
