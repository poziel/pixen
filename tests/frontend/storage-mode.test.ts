import { expect, test } from 'bun:test';
import { createStorageMode } from '../../src/services/storageMode';
import type { call } from '../../src/services/backend';

function fixture(failure = '') {
  const events: string[] = [];
  const service = createStorageMode({
    idle: async () => { events.push('idle'); },
    flush: async () => { events.push('flush'); if (failure === 'flush') throw Error('disk full'); },
    stopChats: async () => { events.push('stop chats'); },
    restarting: () => { events.push('restarting'); },
    resume: () => { events.push('resume'); },
  }, (async (command: string) => {
    events.push(command);
    if (command === failure) throw Error('failed ' + command);
    if (command === 'storage_status') return { mode: 'default', managed: false, paths: {config:'settings'} };
    if (command === 'prepare_storage_migration') return 'ticket';
  }) as typeof call);
  return { service, events };
}
test('migration flushes pending edits before checkpointing and starts helper last', async () => {
  const {service,events} = fixture(); await service.load(); service.state.selected='portable'; await service.apply();
  expect(events).toEqual(['storage_status','idle','flush','stop chats','prepare_storage_migration','restarting','begin_storage_migration']);
  expect(service.state.busy).toBe(true);
});
test('failed preference flush keeps the application open and never prepares a transfer', async () => {
  const {service,events} = fixture('flush'); await service.load(); service.state.selected='semi-portable'; await service.apply();
  expect(events).toEqual(['storage_status','idle','flush','resume']);
  expect(service.state.busy).toBe(false); expect(service.state.error).toContain('disk full');
});
test('helper launch failure cancels the reserved native migration and resumes editing', async () => {
  const {service,events} = fixture('begin_storage_migration'); await service.load(); service.state.selected='portable'; await service.apply();
  expect(events.slice(-3)).toEqual(['begin_storage_migration','cancel_storage_migration','resume']);
  expect(service.state.busy).toBe(false);
});
test('unchanged and managed profiles cannot initiate a migration', async () => {
  const {service,events} = fixture(); await service.load(); await service.apply();
  service.state.selected='portable'; service.state.managed=true; await service.apply();
  expect(events).toEqual(['storage_status']);
});
