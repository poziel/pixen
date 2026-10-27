import { expect, test } from 'bun:test';
import { createLazyAgentConnection } from '../../src/services/lazyAgent';
import type { AgentConnection } from '@frame/core/modules/chat';

test('local chat creation does not connect; the first prompt connects once', async () => {
  let starts = 0;
  const sent: unknown[] = [];
  const native: AgentConnection = { request: async <T>(_m: string,p: unknown) => { sent.push(p); return null as T; }, subscribe: () => () => {} };
  const lazy = createLazyAgentConnection(async () => { starts++; return { connection: native, sessionId: 'real', dispose: async () => {} }; });
  const local = await lazy.connection.request<{sessionId:string}>('session/create', {});
  expect(starts).toBe(0);
  await lazy.connection.request('turn/send', { sessionId: local.sessionId, turnId: 'one' });
  await lazy.connection.request('turn/send', { sessionId: local.sessionId, turnId: 'two' });
  expect(starts).toBe(1);
  expect(sent).toEqual([{sessionId:'real',turnId:'one'},{sessionId:'real',turnId:'two'}]);
  await lazy.dispose();
});

test('failed startup can be retried without losing the local chat', async () => {
  let starts = 0;
  const lazy = createLazyAgentConnection(async () => {
    if (++starts === 1) throw new Error('not signed in');
    return { connection: {request: async <T>() => null as T, subscribe: () => () => {}}, sessionId:'real', dispose:async()=>{} };
  });
  await expect(lazy.connection.request('turn/send', {turnId:'one'})).rejects.toThrow('not signed in');
  await lazy.connection.request('turn/send', {turnId:'two'});
  expect(starts).toBe(2);
  await lazy.dispose();
});

test('sending again during cancelled startup creates a fresh connection', async () => {
  let release!: () => void, starts = 0, sent = 0;
  const wait = new Promise<void>(resolve => { release = resolve; });
  const lazy = createLazyAgentConnection(async () => {
    if (++starts === 1) await wait;
    return { connection: { request: async <T>() => { sent++; return null as T; }, subscribe: () => () => {} }, sessionId:'real', dispose:async()=>{} };
  });
  const first = lazy.connection.request('turn/send', {turnId:'first'}).catch(() => {});
  await lazy.connection.request('turn/cancel', {turnId:'first'});
  const second = lazy.connection.request('turn/send', {turnId:'second'});
  release();
  await first; await second;
  expect(starts).toBe(2);
  expect(sent).toBe(1);
  await lazy.dispose();
});
