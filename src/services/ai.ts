import {
  connectTauriAgent,
  type TauriAgentConnection,
} from '@frame/tauri/agents';
import {shallowReactive, type InjectionKey, inject} from 'vue';
import {isTauri} from '@tauri-apps/api/core';
import {
  createAgentChatSession,
  type AgentConnection,
  type AIChatController,
} from '@frame/core/modules/chat';
import {call} from './backend';
import {queueAgentTurns} from './agentConnection';
import {createLazyAgentConnection} from './lazyAgent';
import type {AiSettings} from './aiSettings';
import {prepareReferences} from './aiReferences';

export interface ArtConversation {

  chat?: AIChatController;

  connecting: boolean;

  error: string;
}

export function createArtChatService(settings: AiSettings) {
  const entries = shallowReactive(new Map<string, ArtConversation>());
  const sessions = new Map<string, {dispose(): Promise<void>}>();
  let disposed = false;
  function entry(documentId: string) {
    let current = entries.get(documentId);
    if (current) return current;
    current = shallowReactive({connecting: false, error: ''});
    entries.set(documentId, current);
    const value = current;
    const lazy = createLazyAgentConnection(async (signal) => {
      if (!isTauri())
        throw new Error('AI chat requires the desktop application.');
      const selected = settings.selection();
      value.connecting = true;
      value.error = '';
      let transport: TauriAgentConnection | undefined;
      const disconnect = async () => {
        await transport?.dispose();
      };
      const abort = () => {
        void disconnect().catch(() => {});
      };
      signal.addEventListener('abort', abort, {once: true});
      try {
        const connected = await connectTauriAgent((events) =>
          call<{runtimeId: string; cwd: string}>('ai_connect', {
            documentId,
            provider: selected.provider,
            events,
          })
        );
        transport = connected.connection;
        const info = connected.info;
        if (signal.aborted) throw new Error('Connection cancelled');
        const connection: AgentConnection = queueAgentTurns(transport);
        const native = await connection.request<{sessionId: string}>(
          'session/create',
          {
            applicationId: 'pixen',
            provider: selected.provider,
            cwd: info.cwd,
            ...(selected.model ? {model: selected.model} : {}),
          }
        );
        if (signal.aborted) throw new Error('Connection cancelled');
        void settings.refresh(selected.provider);
        return {connection, sessionId: native.sessionId, dispose: disconnect};
      } catch (e) {
        await disconnect().catch(() => {});
        throw e;
      } finally {
        signal.removeEventListener('abort', abort);
        value.connecting = false;
      }
    });
    sessions.set(documentId, lazy);
    // This session is local only. The lazy connection starts native work on turn/send.
    void createAgentChatSession(lazy.connection, {
      applicationId: 'pixen',
      provider: settings.state.provider,
      cwd: '',
      prepareAttachments: prepareReferences,
    })
      .then((native) => {
        if (disposed || entries.get(documentId) !== value) {
          native.chat.dispose();
          return;
        }
        value.chat = native.chat;
      })
      .catch((e) => {
        value.error = String(e);
      });
    return value;
  }
  async function close(documentId: string) {
    const current = entries.get(documentId);
    if (current) {
      current.chat?.dispose();
      current.chat = undefined;
      current.connecting = true;
    }
    const session = sessions.get(documentId);
    sessions.delete(documentId);
    await session?.dispose().catch(() => {});
    if (entries.get(documentId) === current) entries.delete(documentId);
  }
  async function retain(ids: string[]) {
    await Promise.all(
      [...entries.keys()].filter((id) => !ids.includes(id)).map(close)
    );
  }
  async function dispose() {
    disposed = true;
    await Promise.all([...entries.keys()].map(close));
  }
  return {entry, close, retain, dispose};
}

export type ArtChatService = ReturnType<typeof createArtChatService>;

export const ArtChatKey: InjectionKey<ArtChatService> = Symbol('Art chat');
export function useArtChat() {
  const value = inject(ArtChatKey);
  if (!value) throw new Error('Art chat service is missing');
  return value;
}
