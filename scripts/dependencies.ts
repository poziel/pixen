import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const frame = resolve(process.env.PIXEN_FRAME_PATH ?? resolve(root, '../frame'));
const server = resolve(process.env.PIXEN_FRAME_SERVER_PATH ?? resolve(root, '../frame-server'));
const state = resolve(root, 'target/dependency-mode.json');
const bun = process.execPath;
let activeChild: ReturnType<typeof Bun.spawn> | undefined;
for (const signal of ['SIGINT', 'SIGTERM'] as const) {
  process.on(signal, () => activeChild?.kill(signal));
}

async function run(args: string[], cwd = root) {
  const child = Bun.spawn(args, { cwd, stdin: 'inherit', stdout: 'inherit', stderr: 'inherit' });
  activeChild = child;
  const code = await child.exited;
  activeChild = undefined;
  if (code !== 0) throw new Error(`Command failed (${code}): ${args.slice(0, 3).join(' ')}`);
}

function record(mode: 'local' | 'released') {
  mkdirSync(dirname(state), { recursive: true });
  writeFileSync(state, JSON.stringify({ mode, frame, server }, null, 2) + '\n');
}

async function installLocal() {
  for (const path of [resolve(frame, 'packages/core/package.json'), resolve(frame, 'packages/tauri/package.json'), resolve(server, 'packages/agents-rust/Cargo.toml')]) {
    if (!existsSync(path)) throw new Error(`Local dependency missing: ${path}. See docs/dependencies.md.`);
  }
  for (const [entry, args] of [
    ['packages/core/dist/frame.js', ['run', 'build:library']],
    ['packages/tauri/dist/index.js', ['run', '--cwd', 'packages/tauri', 'build']],
  ] as const) {
    if (!existsSync(resolve(frame, entry))) await run([bun, ...args], frame);
  }
  const path = resolve(root, 'package.json');
  const original = readFileSync(path, 'utf8');
  const manifest = JSON.parse(original);
  manifest.dependencies['@frame/core'] = `file:${resolve(frame, 'packages/core').replaceAll('\\', '/')}`;
  manifest.dependencies['@frame/tauri'] = `file:${resolve(frame, 'packages/tauri').replaceAll('\\', '/')}`;
  manifest.overrides = { ...manifest.overrides, '@frame/core': manifest.dependencies['@frame/core'] };
  const temporary = JSON.stringify(manifest, null, 2) + '\n';
  writeFileSync(path, temporary);
  try {
    // --no-save keeps the committed release lockfile untouched.
    await run([bun, 'install', '--no-save']);
  } finally {
    if (readFileSync(path, 'utf8') === temporary) writeFileSync(path, original);
    else throw new Error('package.json changed during local setup; preserving that edit. Restore release dependencies before committing.');
  }
  record('local');
  console.log(`Local Frame: ${frame}\nLocal Frame Server: ${server}`);
}

async function installReleased() {
  await run([bun, 'install', '--frozen-lockfile']);
  record('released');
}

async function localCommand(command: string, args: string[]) {
  if (process.env.CI) throw new Error('Local dependencies are disabled in CI. Use released builds.');
  // Refresh copied package outputs; rebuild Frame explicitly after source edits.
  await installLocal();
  const lock = resolve(root, 'src-tauri/Cargo.lock');
  const original = existsSync(lock) ? readFileSync(lock) : undefined;
  const patch = `patch."https://github.com/poziel/frame-server.git".frame-agent-connectors.path=${JSON.stringify(resolve(server, 'packages/agents-rust').replaceAll('\\', '/'))}`;
  try {
    if (command === 'native-check') {
      await run(['cargo', 'check', '--manifest-path', 'src-tauri/Cargo.toml', ...args, '--config', patch]);
    } else {
      const extra = command === 'dev' ? ['dev']
        : command === 'debug' ? ['build', '--debug', '--no-bundle']
        : ['build', '--no-bundle'];
      await run([bun, 'run', 'tauri', ...extra, ...args, '--', '--config', patch]);
    }
  } finally {
    // Cargo records path patches in its lockfile. Keep that local resolution out
    // of the committed release lock after the native process has exited.
    if (original) writeFileSync(lock, original);
    else rmSync(lock, { force: true });
  }
}

const [command, ...args] = process.argv.slice(2);
switch (command) {
  case 'local': await installLocal(); break;
  case 'released': await installReleased(); break;
  case 'dev': case 'debug': case 'native-check': case 'build': await localCommand(command, args); break;
  case 'released-build':
    await installReleased();
    await run([bun, 'run', 'tauri', 'build', '--no-bundle', ...args, '--', '--locked']);
    break;
  default: throw new Error('Usage: bun scripts/dependencies.ts local|released|dev|debug|native-check|build|released-build');
}
