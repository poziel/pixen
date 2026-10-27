import { expect, test } from 'bun:test';
import { readFileSync, existsSync } from 'node:fs';

test('Pixen desktop and web entry points share the new branding assets', () => {
 const config = JSON.parse(readFileSync('src-tauri/tauri.conf.json','utf8'));
 expect(config.productName).toBe('Pixen');
 expect(config.identifier).toBe('com.poziel.pixen');
 expect(config.app.windows[0].title).toBe('Pixen');
 expect(config.bundle.icon).toEqual(['../assets/pixen.ico']);
 expect(existsSync('assets/pixen.ico')).toBe(true);
 const ico = readFileSync('assets/pixen.ico');
 expect(ico.readUInt16LE(0)).toBe(0);
 expect(ico.readUInt16LE(2)).toBe(1);
 expect(ico.readUInt16LE(4)).toBeGreaterThan(0);
 expect(readFileSync('index.html','utf8')).toContain('assets/pixen.svg');
 expect(readFileSync('src/App.vue','utf8')).toContain('../assets/pixen.svg');
 expect(readFileSync('build.rs','utf8')).toContain('.set_icon("assets/pixen.ico")');
});
