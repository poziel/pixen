<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref } from 'vue';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { call } from '../services/backend';
import { AIChat } from '@frame/core/modules/chat';
import { useEditor } from '../editor/editor';
import { useArtChat } from '../services/ai';
import { defaultAiWelcome } from '../services/aiSettings';
const editor = useEditor(), service = useArtChat();
const documentId = computed(() => editor.state.snapshot?.active_document ?? undefined);
const conversation = computed(() => documentId.value ? service.entry(documentId.value) : undefined);
const managed = computed(() => editor.state.snapshot?.managed);
const region=ref<HTMLElement>(); let disposed=false, unlisten:(()=>void)|undefined;
onMounted(async () => {
  if (!isTauri()) return;
  const stop=await getCurrentWindow().onDragDropEvent(async ({payload}) => {
    if (disposed || payload.type!=='drop' || managed.value || !conversation.value?.chat) return;
    const bounds=region.value?.getBoundingClientRect(); if (!bounds || !bounds.width || !bounds.height) return;
    const factor=globalThis.devicePixelRatio || 1, x=payload.position.x/factor, y=payload.position.y/factor;
    if (x<bounds.left || x>=bounds.right || y<bounds.top || y>=bounds.bottom) return;
    const current=conversation.value;
    try {
      const references=await call<{name:string;data:string}[]>('ai_drop_files',{paths:payload.paths});
      if (disposed || current!==conversation.value || !current.chat) return;
      const files=references.map(r => new File([Uint8Array.from(atob(r.data),char=>char.charCodeAt(0))],r.name));
      current.chat.state.attachments.push(...files.map(file=>({id:crypto.randomUUID(),name:file.name,kind:'file' as const,files:[file]})));
      current.error='';
    } catch(e) { current.error=String(e); }
  });
  if (disposed) stop(); else unlisten=stop;
});
onBeforeUnmount(()=>{disposed=true;unlisten?.();});
</script>
<template>
  <section ref="region" class="art-chat" aria-label="AI art chat">
    <p v-if="!documentId || managed" class="art-chat-note">{{ managed ? 'AI editing is unavailable in managed previews.' : 'Open or create artwork to start an AI conversation.' }}</p>
    <template v-else-if="conversation">
      <p v-if="conversation.connecting" class="art-chat-note" role="status">Connecting your AI assistant…</p>
      <p v-if="conversation.error" class="art-chat-note art-chat-error" role="alert">{{ conversation.error }}</p>
      <AIChat v-if="conversation.chat" :chat="conversation.chat" :empty-message="defaultAiWelcome" :allow-rewind="false" :allow-fork="false" placeholder="Describe the art you want to create…">
      </AIChat>
    </template>
  </section>
</template>
<style scoped>
.art-chat { display: flex; flex-direction: column; height: 100%; min-height: 0; }
.art-chat :deep(.frame-ai-view) { flex: 1; min-height: 0; }
.art-chat-note { margin: 0; padding: 12px; line-height: 1.5; color: var(--frame-muted); }
.art-chat-error { color: var(--frame-error, #e88080); overflow-wrap: anywhere; }
</style>
