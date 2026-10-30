/**
 * @fileoverview Writes the platform-specific Tauri bundling override used by CI.
 *
 * Run after generating icons under target/ci-icons. This creates configuration;
 * it does not build an installer, sign with a production identity, or notarize.
 */

import {mkdirSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';

/** Subset of Tauri's bundle configuration supplied by the release workflow. */
interface ReleaseBundleOptions {

  /** Enable the installers selected by the platform's CI job. */
  readonly active: boolean;

  /** Absolute platform icon paths, including PNG for compile-time loading. */
  readonly icon: string[];

  /** macOS signing override; "-" requests ad-hoc signing for preview packages. */
  macOS?: {readonly signingIdentity: string};
}

// Resolve the platform icon produced by the earlier Tauri icon-generation step.
mkdirSync('target', {recursive: true});
const iconDirectory = resolve('target/ci-icons');
const platformIcon = process.platform === 'win32' ? 'icon.ico' :
  process.platform === 'darwin' ? 'icon.icns' : '128x128.png';
const bundle: ReleaseBundleOptions = {
  active: true,
  // Tauri's macOS context also needs a PNG alongside its installer ICNS.
  icon: [resolve(iconDirectory, platformIcon), resolve(iconDirectory, 'icon.png')],
};

// Ad-hoc signing permits local testing; Developer ID notarization is separate.
if (process.platform === 'darwin') {
  bundle.macOS = {signingIdentity: '-'};
}
writeFileSync(
  'target/tauri.release.json', JSON.stringify({bundle}, null, 2) + '\n'
);
