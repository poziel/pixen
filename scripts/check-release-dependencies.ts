import { readFileSync } from 'node:fs';

// Fail before starting expensive native jobs until published dependencies exist.
const manifest = JSON.parse(readFileSync('package.json', 'utf8'));
const local = Object.entries({
  ...manifest.dependencies,
  ...manifest.devDependencies,
  ...manifest.overrides,
}).filter(([, value]) => /^(file:|link:|workspace:)/.test(String(value)));
const nativeManifest = readFileSync('src-tauri/Cargo.toml', 'utf8');
const native = nativeManifest.match(/^frame-agent-connectors\s*=.*$/m)?.[0];
if (local.length || !native || /\bpath\s*=/.test(native)) {
  console.error('Desktop releases require published Frame and Frame Native dependencies.');
  for (const [name] of local) console.error(`Replace the local dependency: ${name}`);
  if (!native || /\bpath\s*=/.test(native)) console.error('Replace frame-agent-connectors with a pinned Cargo registry or Git dependency.');
  console.error('Update and commit bun.lock and src-tauri/Cargo.lock. See docs/releases.md.');
  process.exit(1);
}
const version = manifest.version;
const tauri = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
for (const path of ['Cargo.toml', 'src-tauri/Cargo.toml']) {
  const cargoVersion = readFileSync(path, 'utf8').match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (cargoVersion !== version) throw new Error(`${path} version must match package.json`);
}
if (tauri.version !== version) throw new Error('Tauri version must match package.json');
if (process.env.GITHUB_REF_TYPE === 'tag' && process.env.GITHUB_REF_NAME !== `v${version}`) {
  throw new Error('Release tag must equal v<package.json version>');
}
console.log(`Release dependencies and version ${version} are configured.`);
