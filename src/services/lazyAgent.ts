import type { AgentConnection, AgentEventEnvelope, AgentSession } from '@frame/core/modules/chat';

interface NativeChat { connection: AgentConnection; sessionId: string; dispose(): Promise<void> }
/** Frame can render a local conversation before any provider process exists. */
export function createLazyAgentConnection(start: (signal: AbortSignal) => Promise<NativeChat>) {
  const localId = crypto.randomUUID();
  const listeners = new Set<(event: AgentEventEnvelope) => void>();
  const cancelled = new Set<string>();
  let ready: NativeChat | undefined, pending: Promise<NativeChat> | undefined;
  let starting: AbortController | undefined, unsubscribe: (() => void) | undefined, disposed = false;
  async function ensureReady(turnId?: string): Promise<NativeChat> {
    if (disposed) throw new Error('Conversation is closed');
    if (turnId && cancelled.delete(turnId)) throw new Error('Turn cancelled');
    if (ready) return ready;
    if (pending && starting?.signal.aborted) {
      await pending.catch(() => {});
      return ensureReady(turnId);
    }
    if (!pending) {
      starting = new AbortController();
      const signal = starting.signal;
      pending = start(signal).then(async native => {
        if (disposed || signal.aborted) { await native.dispose(); throw new Error('Connection cancelled'); }
        ready = native;
        unsubscribe = native.connection.subscribe(event => {
          for (const listener of listeners) listener({ ...event, sessionId: localId });
        });
        return native;
      }).catch(error => { pending = undefined; throw error; });
    }
    return pending;
  }
  async function dispose() {
    if (disposed) return;
    disposed = true; starting?.abort(); unsubscribe?.(); listeners.clear();
    if (ready) await ready.dispose();
  }
  const connection: AgentConnection = {
    subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener); }; },
    async request<T>(method: string, params: unknown): Promise<T> {
      const value = params as Record<string, unknown>;
      if (disposed) throw new Error('Conversation is closed');
      if (method === 'session/create') return { sessionId: localId, providerSessionId: localId, provider: value.provider, capabilities: {} } as AgentSession as T;
      if (method === 'session/close') { await dispose(); return null as T; }
      if (method === 'turn/cancel') {
        if (!ready) { if (typeof value.turnId === 'string') cancelled.add(value.turnId); starting?.abort(); return null as T; }
      }
      if (method === 'turn/send') {
        const native = await ensureReady(typeof value.turnId === 'string' ? value.turnId : undefined);
        if (typeof value.turnId === 'string' && cancelled.delete(value.turnId)) throw new Error('Turn cancelled');
        return native.connection.request<T>(method, { ...value, sessionId: native.sessionId });
      }
      if (!ready) throw new Error('Provider is not connected');
      return ready.connection.request<T>(method, { ...value, sessionId: ready.sessionId });
    },
  };
  return { connection, dispose };
}
