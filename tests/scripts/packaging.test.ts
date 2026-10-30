import { afterEach, expect, test } from 'bun:test';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve, sep } from 'node:path';
import { unzipSync, zipSync } from 'fflate';

const roots: string[] = [];
function fixture() {
  mkdirSync('target', { recursive: true });
  const root = mkdtempSync(resolve('target/packaging-test-'));
  roots.push(root);
  for (const path of ['target/release', 'assets/themes', 'playbook/skills/pixen', 'playbook/skills/pixel-art']) mkdirSync(join(root, path), { recursive: true });
  writeFileSync(join(root, 'package.json'), JSON.stringify({ version: '0.2.0' }));
  writeFileSync(join(root, 'LICENSE'), 'license');
  writeFileSync(join(root, 'assets/themes/pixen.yaml'), 'id: pixen\nname: Pixen\n');
  for (const name of ['pixen', 'pixen-cli', 'pixen-mcp']) for (const suffix of ['', '.exe']) writeFileSync(join(root, 'target/release', name + suffix), name, { mode: 0o755 });
  for (const name of ['pixen', 'pixel-art']) writeFileSync(join(root, 'playbook/skills', name, 'SKILL.md'), '# Skill\n');
  return root;
}
afterEach(() => {
  for (const root of roots.splice(0)) {
    expect(root.startsWith(resolve('target') + sep)).toBe(true);
    rmSync(root, { recursive: true, force: true });
  }
});

test('Windows release includes desktop, tools, skills, and hashes without packaging user data', async () => {
  const { packageRelease } = await import('../../scripts/release');
  const { verifyWindowsPackage } = await import('../../scripts/verify-windows-package');
  const root = fixture();
  const existing = join(root, 'target/portable/Pixen-0.2.0-x86_64-pc-windows-msvc');
  mkdirSync(join(existing, 'data'), { recursive: true });
  writeFileSync(join(existing, 'data/settings.json'), 'personal');
  writeFileSync(join(existing, 'portable'), '');
  const result = await packageRelease({ root, target: 'x86_64-pc-windows-msvc', packageOnly: true });
  const entries = unzipSync(readFileSync(result.archive));
  expect(Object.keys(entries).filter(name => name.endsWith('.exe'))).toHaveLength(3);
  expect(Object.keys(entries).some(name => name.endsWith('/skills/pixen/SKILL.md'))).toBe(true);
  expect(Object.keys(entries).some(name => name.includes('/data/') || name.endsWith('/portable'))).toBe(false);
  expect(readFileSync(join(existing, 'data/settings.json'), 'utf8')).toBe('personal');
  expect(result.directory).not.toBe(existing);
  expect(verifyWindowsPackage(result.archive)).toBeUndefined();
  const sums = new TextDecoder().decode(entries[Object.keys(entries).find(name => name.endsWith('/SHA256SUMS.txt'))!]);
  expect(sums.match(/^[a-f0-9]{64}  pixen.*\.exe$/gm)).toHaveLength(3);
});

test('tools package excludes desktop and packages both binaries with their skills', async () => {
  const { packageTools } = await import('../../scripts/package-tools');
  const root = fixture();
  const archive = await packageTools({ root, target: 'x86_64-pc-windows-msvc' });
  const files = Object.keys(unzipSync(readFileSync(archive)));
  expect(files.filter(name => name.endsWith('.exe'))).toHaveLength(2);
  expect(files.some(name => name.endsWith('/pixen.exe'))).toBe(false);
  expect(files.some(name => name.endsWith('/LICENSE'))).toBe(true);
  expect(files.some(name => name.endsWith('/skills/pixel-art/SKILL.md'))).toBe(true);
  const listing = Bun.spawnSync(['tar', '-tf', archive]);
  expect(listing.exitCode).toBe(0);
  expect(listing.stdout.toString()).toContain('/pixen-cli.exe');
});

test('non-Windows application packaging produces a tarball with the desktop and tools', async () => {
  const { packageRelease } = await import('../../scripts/release');
  const root = fixture();
  const result = await packageRelease({ root, target: 'aarch64-apple-darwin', packageOnly: true });
  const listing = Bun.spawnSync(['tar', '-tzf', result.archive]);
  expect(listing.exitCode).toBe(0);
  const files = listing.stdout.toString().split(/\r?\n/);
  for (const binary of ['pixen', 'pixen-cli', 'pixen-mcp']) expect(files.some(name => name.endsWith('/' + binary))).toBe(true);
});

test('Linux tools archive contains both tools and retains executable permissions on POSIX', async () => {
  const { packageTools } = await import('../../scripts/package-tools');
  const root = fixture();
  const archive = await packageTools({ root, target: 'x86_64-unknown-linux-gnu' });
  const listing = Bun.spawnSync(['tar', '-tvzf', archive]);
  expect(listing.exitCode).toBe(0);
  expect(listing.stdout.toString()).toContain('/pixen-cli');
  expect(listing.stdout.toString()).toContain('/pixen-mcp');
  if (process.platform !== 'win32') {
    expect(listing.stdout.toString()).toMatch(/rwx.*pixen-cli/);
    expect(listing.stdout.toString()).toMatch(/rwx.*pixen-mcp/);
  }
});

test('archive verifier rejects missing, empty, or separated executables', async () => {
  const { verifyWindowsPackage } = await import('../../scripts/verify-windows-package');
  const root = fixture();
  const invalid: Record<string, Uint8Array>[] = [
    { 'app/pixen.exe': new Uint8Array([1]) },
    { 'app/pixen.exe': new Uint8Array(), 'app/pixen-cli.exe': new Uint8Array([1]), 'app/pixen-mcp.exe': new Uint8Array([1]) },
    { 'app/pixen.exe': new Uint8Array([1]), 'other/pixen-cli.exe': new Uint8Array([1]), 'app/pixen-mcp.exe': new Uint8Array([1]) },
  ];
  for (const files of invalid) {
    const archive = join(root, 'bad.zip');
    writeFileSync(archive, zipSync(files));
    expect(() => verifyWindowsPackage(archive)).toThrow();
  }
});

test('packaging rejects invalid targets and missing binaries', async () => {
  const { packageTools } = await import('../../scripts/package-tools');
  const root = fixture();
  await expect(packageTools({ root, target: '../escape' })).rejects.toThrow('target');
  rmSync(join(root, 'target/release/pixen-mcp.exe'));
  await expect(packageTools({ root, target: 'x86_64-pc-windows-msvc' })).rejects.toThrow('pixen-mcp.exe');
});
