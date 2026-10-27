import { reactive } from 'vue';
import { call } from './backend';
export type StorageMode = 'default' | 'portable' | 'semi-portable';
export interface StorageStatus { mode: StorageMode; managed: boolean; paths: Record<string, string> }
export interface StorageMigrationHooks { idle(): Promise<unknown>; flush(): Promise<unknown>; stopChats(): Promise<unknown>; restarting(): void; resume(): void }

export function createStorageMode(hooks: StorageMigrationHooks, transport: typeof call = call) {
  const state = reactive({ loaded: false, mode: 'default' as StorageMode, selected: 'default' as StorageMode, managed: false, paths: {} as Record<string,string>, busy: false, error: '' });
  async function load() {
    try { const value=await transport<StorageStatus>('storage_status'); if (!value?.mode) return; state.mode=value.mode;state.selected=value.mode;state.managed=value.managed;state.paths=value.paths;state.loaded=true; }
    catch(error){state.error=String(error);}
  }
  async function apply() {
    if (state.busy || state.managed || !state.loaded || state.selected===state.mode) return;
    state.busy=true;state.error='';let directory:string|undefined;
    try {
      await hooks.idle();await hooks.flush();await hooks.stopChats();
      directory=await transport<string>('prepare_storage_migration',{target:state.selected});
      hooks.restarting();
      await transport('begin_storage_migration',{directory});
    } catch(error) {
      if (directory) await transport('cancel_storage_migration').catch(()=>{});
      hooks.resume();state.error=String(error);state.busy=false;
    }
  }
  return {state,load,apply};
}
export type StorageModeService=ReturnType<typeof createStorageMode>;
