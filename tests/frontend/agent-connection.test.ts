import { expect, test } from 'bun:test';
import { queueAgentTurns } from '../../src/services/agentConnection';
import type { AgentConnection } from '@frame/core/modules/chat';

function transport() {
  const calls: string[] = [];
  let settle!: () => void;
  const pending = new Promise<void>(resolve => { settle = resolve; });
  const connection: AgentConnection = {
    async request<T>(method: string, params: unknown): Promise<T> {
      const id = (params as { turnId: string }).turnId;
      calls.push(`${method}:${id}`);
      if (method === 'turn/send' && id === 'first') await pending;
      return null as T;
    },
    subscribe: () => () => {},
  };
  return { connection: queueAgentTurns(connection), calls, settle };
}

test('a prompt sent immediately after Stop waits for native cancellation to settle', async () => {
  const { connection, calls, settle } = transport();
  const first = connection.request('turn/send', { turnId: 'first' });
  await Promise.resolve();
  await connection.request('turn/cancel', { turnId: 'first' });
  const second = connection.request('turn/send', { turnId: 'second' });
  await Promise.resolve();
  expect(calls).toEqual(['turn/send:first', 'turn/cancel:first']);
  settle(); await first; await second;
  expect(calls.at(-1)).toBe('turn/send:second');
});

test('stopping a queued prompt never sends that prompt to the provider', async () => {
  const { connection, calls, settle } = transport();
  const first = connection.request('turn/send', { turnId: 'first' });
  await Promise.resolve();
  const second = connection.request('turn/send', { turnId: 'second' }).catch(e => e.message);
  await connection.request('turn/cancel', { turnId: 'second' });
  settle(); await first;
  expect(await second).toContain('cancelled');
  expect(calls).toEqual(['turn/send:first']);
});
