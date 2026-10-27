import type { AgentConnection } from '@frame/core/modules/chat';

/** Stop releases the composer before native cancellation finishes. Keep new turns in order. */
export function queueAgentTurns(connection: AgentConnection): AgentConnection {
  let previous: Promise<unknown> = Promise.resolve();
  const turns = new Map<string, { started: boolean; cancelled: boolean }>();
  return {
    subscribe: listener => connection.subscribe(listener),
    request<T>(method: string, params: unknown): Promise<T> {
      const turnId = (params as { turnId?: string } | null)?.turnId;
      if (method === 'turn/cancel' && turnId) {
        const turn = turns.get(turnId);
        if (turn && !turn.started) { turn.cancelled = true; return Promise.resolve(null as T); }
      }
      if (method === 'session/close') for (const turn of turns.values()) turn.cancelled = true;
      if (method !== 'turn/send' || !turnId) return connection.request<T>(method, params);
      const turn = { started: false, cancelled: false };
      turns.set(turnId, turn);
      const result = previous.then(() => {
        if (turn.cancelled) throw new Error('Turn was cancelled before it started');
        turn.started = true;
        return connection.request<T>(method, params);
      }).finally(() => { turns.delete(turnId); });
      previous = result.catch(() => {});
      return result;
    },
  };
}
