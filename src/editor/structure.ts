import { computed, inject, ref, watch, type InjectionKey } from 'vue';
import type { Editor } from './editor';

export type StructureKind = 'layer' | 'frame';
export function createStructure(editor: Editor, kind: StructureKind) {
  const s = editor.state;
  const doc = computed(() => s.snapshot?.document);
  const items = computed(() => kind === 'layer' ? doc.value?.layers ?? [] : doc.value?.frames ?? []);
  const display = computed(() => kind === 'layer' ? [...items.value].reverse() : items.value);
  const active = computed(() => kind === 'layer' ? s.layer : doc.value?.frames[s.frame]?.id);
  const disabled = computed(() => !doc.value || s.working || !!s.snapshot?.managed || !!doc.value.alternate_view);
  const selection = ref<string[]>([]), editing = ref<string | null>(null);
  const deletion = ref<{ id: string; name: string; document: string | null | undefined; generation: number } | null>(null);
  watch(active, id => { if (id && !selection.value.includes(id)) selection.value = [id]; }, { immediate: true });
  watch(() => s.snapshot?.active_document, () => { selection.value = active.value ? [active.value] : []; editing.value = null; deletion.value = null; });
  watch(items, value => { selection.value = selection.value.filter(id => value.some(item => item.id === id)); });
  function name(id: string) { const index = items.value.findIndex(item => item.id === id); return items.value[index]?.name || `${kind === 'layer' ? 'Layer' : 'Frame'} ${index + 1}`; }
  function select(id: string) { if (editor.following()) return; if (kind === 'layer') s.layer = id; else editor.select(items.value.findIndex(item => item.id === id)); }
  async function edit(operation: unknown) {
    if (disabled.value || !doc.value) return;
    s.playing = false;
    await editor.action('edit_structure', { documentId: s.snapshot?.active_document, generation: doc.value.generation, operation });
  }
  async function add(at?: string, empty=false) {
    const id = `${kind}-${crypto.randomUUID()}`;
    const position = items.value.findIndex(item => item.id === (at ?? active.value));
    const index = position < 0 ? items.value.length : position + (kind === 'frame' ? 1 : 0);
    let number = items.value.length + 1;
    while (items.value.some(item => item.name === `Layer ${number}`)) number++;
    await edit(kind === 'layer' ? { type: 'add_layer', layer_id: id, name: `Layer ${number}`, index }
      : !empty&&position>=0 ? {type:'duplicate_frame',frame_id:items.value[position]!.id,new_frame_id:id,index}
      : { type: 'add_frame', frame_id: id, duration_ms: doc.value?.frames[position]?.duration_ms ?? 100, index });
    if (items.value.some(item => item.id === id)) { select(id); selection.value = [id]; }
  }
  function rename(id: string, value: string) {
    editing.value = null;
    if (value.trim() && value.trim() !== name(id)) void edit({ type: `rename_${kind}`, [`${kind}_id`]: id, name: value.trim() });
  }
  async function duplicate(source: string) {
    const id = `${kind}-${crypto.randomUUID()}`, index = items.value.findIndex(item => item.id === source) + (kind === 'frame' ? 1 : 0);
    let suffix = ' copy', count = 2;
    while (items.value.some(item => item.name === name(source) + suffix)) suffix = ` copy ${count++}`;
    const value = name(source) + suffix;
    await edit(kind === 'layer' ? { type: 'duplicate_layer', layer_id: source, new_layer_id: id, name: value, index }
      : [{ type: 'duplicate_frame', frame_id: source, new_frame_id: id, index }, { type: 'rename_frame', frame_id: id, name: value }]);
    if (items.value.some(item => item.id === id)) { select(id); selection.value = [id]; }
  }
  function requestDelete(id = active.value) {
    if (!id || disabled.value || items.value.length < 2 || !doc.value) return;
    deletion.value = { id, name: name(id), document: s.snapshot?.active_document, generation: doc.value.generation };
  }
  function confirmDelete() {
    const target = deletion.value; deletion.value = null;
    if (!target || target.document !== s.snapshot?.active_document || target.generation !== doc.value?.generation) return;
    void edit({ type: `remove_${kind}`, [`${kind}_id`]: target.id });
  }
  function reorder(order: string[]) {
    const native = kind === 'layer' ? [...order].reverse() : order;
    if (native.length !== items.value.length || new Set(native).size !== native.length || native.some(id => !items.value.some(item => item.id === id))) return;
    void edit(native.map((id, index) => ({ type: `reorder_${kind}`, [`${kind}_id`]: id, index })));
  }
  return { kind, items, display, active, disabled, selection, editing, deletion, name, select, add, addEmpty:(at?:string)=>add(at,true), rename, duplicate, requestDelete, confirmDelete, reorder };
}
export type Structure = ReturnType<typeof createStructure>;
export const StructureKey: InjectionKey<Record<StructureKind, Structure>> = Symbol('Pixen structure');
export function useStructure(kind: StructureKind) { const value = inject(StructureKey); if (!value) throw new Error('Missing structure controller'); return value[kind]; }
