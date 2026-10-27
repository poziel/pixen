import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

mkdirSync('target', { recursive: true });
const root = resolve('target/ci-icons');
const icon = process.platform === 'win32' ? 'icon.ico'
  : process.platform === 'darwin' ? 'icon.icns' : '128x128.png';
const bundle: Record<string, unknown> = {
  active: true,
  // Tauri's macOS compile-time context requires a PNG as well as the bundle ICNS.
  icon: [resolve(root, icon), resolve(root, 'icon.png')],
};
// Ad-hoc signing permits local testing; Developer ID notarization is separate.
if (process.platform === 'darwin') bundle.macOS = { signingIdentity: '-' };
writeFileSync('target/tauri.release.json', JSON.stringify({ bundle }, null, 2) + '\n');
