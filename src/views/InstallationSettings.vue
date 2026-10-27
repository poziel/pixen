<script setup lang="ts">
import { FrameButton, FrameSelect } from '@frame/core';
import type { StorageModeService } from '../services/storageMode';
defineProps<{ storage: StorageModeService }>();
const choices=[{value:'default',label:'Default'},{value:'portable',label:'Portable'},{value:'semi-portable',label:'Semi-portable'}];
const labels: Record<string,string>={config:'Configuration',recovery:'Recovery and unfinished artwork',themes:'Custom themes',webview:'Browser profile',cache:'Application cache',temp:'Temporary workspaces',logs:'Logs'};
</script>
<template>
  <div class="installation-settings">
    <label for="pixen-storage-mode" class="setting-label">Storage mode</label>
    <FrameSelect id="pixen-storage-mode" v-model="storage.state.selected" label="Storage mode" :options="choices" :disabled="!storage.state.loaded || storage.state.managed || storage.state.busy" />
    <p class="setting-description">Default uses this computer’s user folders. Portable keeps all Pixen data beside the app. Semi-portable keeps configuration, customizations, and recovery beside the app while browser data, caches, temporary workspaces, and logs stay on this computer.</p>
    <p v-if="storage.state.managed" role="status">Change storage from the standalone editor after closing MCP previews.</p>
    <p class="setting-description">Apply transfers your existing data and restarts Pixen. Open artwork reopens without changing saved project files. Undo history and AI conversations reset. Original files and conflicting destination data are retained as backups. Close other Pixen windows and MCP connections first.</p>
    <FrameButton label="Apply and restart" variant="primary" :disabled="!storage.state.loaded || storage.state.managed || storage.state.busy || storage.state.selected === storage.state.mode" @click="storage.apply()" />
    <p v-if="storage.state.error" role="alert" class="installation-error">{{ storage.state.error }}</p>
    <h3>Current locations</h3>
    <dl><template v-for="(label,key) in labels" :key="key"><dt>{{ label }}</dt><dd>{{ storage.state.paths[key] ?? 'Loading…' }}</dd></template></dl>
  </div>
</template>
<style scoped>
.installation-settings { display:flex;flex-direction:column;gap:12px; }
.installation-settings dl { margin:0; }
.installation-settings dt { font-weight:600; }
.installation-settings dd { margin:4px 0 14px;overflow-wrap:anywhere;color:var(--frame-muted); }
.installation-error { color:var(--frame-danger); }
</style>
