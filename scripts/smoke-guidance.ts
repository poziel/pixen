/**
 * @fileoverview Windows ZIP install smoke check for the bundled MCP playbook.
 *
 * Extracts a package into a fresh directory, launches its MCP binary from an
 * unrelated directory, and compares resource reads with installed bundle bytes.
 * Run: bun scripts/smoke-guidance.ts --archive <application-or-tools.zip>
 */
import {mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {dirname, isAbsolute, join, resolve, sep} from 'node:path';
import {tmpdir} from 'node:os';
import {unzipSync} from 'fflate';
import {readRequiredOption} from './packaging';

/** Observable results from the installed MCP server. */
export interface GuidanceSmokeResult {
  readonly skill: string;
  readonly rule: string;
  readonly resources: number;
}

/**
 * Verifies a Windows ZIP using only its executable and installed playbook.
 * All extracted files and the child process are removed on success or failure.
 */
export async function smokeGuidance(archivePath: string): Promise<GuidanceSmokeResult> {
  const entries = unzipSync(readFileSync(archivePath));
  // Validate every path before writing anything from the archive.
  for (const name of Object.keys(entries)) {
    if (isAbsolute(name) || name.includes('\\') || name.includes(':') ||
        name.split('/').some(part => part === '..' || part === '.')) {
      throw new Error(`Invalid package path: ${name}`);
    }
  }
  const binaries = Object.keys(entries).filter(name => name.endsWith('/pixen-mcp.exe'));
  if (binaries.length !== 1) throw new Error('Expected one packaged pixen-mcp.exe');
  const prefix = binaries[0].slice(0, -'pixen-mcp.exe'.length);
  for (const file of ['skills/draw/SKILL.md', 'rules/00-core-principles.md', 'LICENSE', 'NOTICE.md']) {
    if (!entries[prefix + 'playbook/' + file]?.length) {
      throw new Error(`Missing installed playbook/${file}`);
    }
  }
  const temporaryRoot = mkdtempSync(join(tmpdir(), 'pixen-guidance-smoke-'));
  let child: ReturnType<typeof Bun.spawn<'pipe', 'pipe', 'pipe'>> | undefined;
  const pending = new Map<number, {resolve(value: any): void; reject(error: Error): void}>();
  try {
    const installation = join(temporaryRoot, 'installation');
    const cwd = join(temporaryRoot, 'unrelated-working-directory');
    mkdirSync(cwd, {recursive: true});
    for (const [name, bytes] of Object.entries(entries)) {
      const destination = resolve(installation, name);
      if (!destination.startsWith(resolve(installation) + sep)) {
        throw new Error(`Invalid extraction destination: ${name}`);
      }
      if (name.endsWith('/')) mkdirSync(destination, {recursive: true});
      else {
        mkdirSync(dirname(destination), {recursive: true});
        writeFileSync(destination, bytes);
      }
    }
    child = Bun.spawn([join(installation, binaries[0])], {cwd, stdin: 'pipe', stdout: 'pipe', stderr: 'pipe'});
    const process = child;
    let diagnostics = '';
    const stderr = (async () => {
      const reader = process.stderr.getReader();
      const decoder = new TextDecoder();
      for (;;) {
        const {value, done} = await reader.read();
        if (done) break;
        diagnostics = (diagnostics + decoder.decode(value, {stream: true})).slice(-4096);
      }
    })();
    const output = (async () => {
      const reader = process.stdout.getReader();
      const decoder = new TextDecoder();
      let buffer = '';
      try {
        for (;;) {
          const {value, done} = await reader.read();
          if (done) break;
          buffer += decoder.decode(value, {stream: true});
          let newline: number;
          while ((newline = buffer.indexOf('\n')) >= 0) {
            const line = buffer.slice(0, newline).trim();
            buffer = buffer.slice(newline + 1);
            if (!line) continue;
            const response = JSON.parse(line);
            const request = pending.get(response.id);
            if (!request) continue;
            pending.delete(response.id);
            if (response.error) request.reject(new Error(JSON.stringify(response.error)));
            else request.resolve(response.result);
          }
        }
      } catch (error) {
        for (const request of pending.values()) request.reject(new Error(String(error)));
      } finally {
        for (const request of pending.values()) request.reject(new Error(`MCP exited: ${diagnostics}`));
        pending.clear();
      }
    })();
    let id = 0;
    async function request(method: string, params: object): Promise<any> {
      const requestId = ++id;
      let timer: ReturnType<typeof setTimeout> | undefined;
      try {
        return await new Promise((resolve, reject) => {
          pending.set(requestId, {resolve, reject});
          timer = setTimeout(() => reject(new Error(`MCP timeout: ${method}; ${diagnostics}`)), 15_000);
          process.stdin.write(JSON.stringify({jsonrpc: '2.0', id: requestId, method, params}) + '\n');
        });
      } finally {
        clearTimeout(timer);
        pending.delete(requestId);
      }
    }
    const initialized = await request('initialize', {protocolVersion: '2025-11-25', capabilities: {}, clientInfo: {name: 'pixen-install-smoke', version: '1'}});
    if (!initialized.capabilities.resources || !initialized.capabilities.prompts) throw new Error('Missing guidance capabilities');
    process.stdin.write(JSON.stringify({jsonrpc: '2.0', method: 'notifications/initialized'}) + '\n');
    const listed = await request('resources/list', {});
    const skill = 'skill://draw';
    const read = await request('resources/read', {uri: skill});
    const skillText = read.contents[0].text as string;
    const rule = skillText.match(/rules:\/\/[a-z0-9-]+/)?.[0];
    if (!rule) throw new Error('Draw workflow must reference a rule');
    const ruleRead = await request('resources/read', {uri: rule});
    const tool = await request('tools/call', {name: 'read_guidance', arguments: {uri: rule}});
    const prompt = await request('prompts/get', {name: 'pixen:draw'});
    const installed = (path: string): string => readFileSync(join(installation, prefix, 'playbook', path), 'utf8').replaceAll('\r\n', '\n');
    if (skillText !== installed('skills/draw/SKILL.md') ||
        ruleRead.contents[0].text !== installed('rules/' + rule.slice('rules://'.length) + '.md') ||
        tool.isError || tool.content[0].text !== ruleRead.contents[0].text ||
        !prompt.messages[0].content.text.includes(skillText)) {
      throw new Error('Installed guidance does not match the MCP catalog');
    }
    process.stdin.end();
    let shutdownTimer: ReturnType<typeof setTimeout> | undefined;
    try {
      await Promise.race([
        Promise.all([output, stderr, process.exited]),
        new Promise((_, reject) => {
          shutdownTimer = setTimeout(() => reject(new Error('MCP did not exit after closing stdin')), 5_000);
        }),
      ]);
    } finally {
      clearTimeout(shutdownTimer);
    }
    if (await process.exited !== 0) throw new Error(`MCP failed: ${diagnostics}`);
    return {skill, rule, resources: listed.resources.length};
  } finally {
    child?.kill();
    if (child) await child.exited;
    rmSync(temporaryRoot, {recursive: true, force: true});
  }
}

if (import.meta.main) {
  console.log(await smokeGuidance(readRequiredOption(process.argv.slice(2), '--archive')));
}
