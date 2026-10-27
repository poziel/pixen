import { computed, onBeforeUnmount, ref, watch } from 'vue';
import type { Editor } from '../editor';
import { cellKey, dragCells, selectCell, type MatrixCell } from './matrixSelection';

/** Pointer capture and hit testing are confined to this application-owned grid. */
export function useMatrixSelection(editor: Editor, clear: () => void) {
  const s = editor.state, root = ref<HTMLElement>();
  const rowIds = computed(() => [...(s.snapshot?.document?.layers ?? [])].reverse().map(l => l.id));
  const frameIds = computed(() => s.snapshot?.document?.frames.map(f => f.id) ?? []);
  const selected = ref<MatrixCell[]>([]), anchor = ref<MatrixCell>();
  const selectedKeys = computed(() => new Set(selected.value.map(cellKey)));
  let internal = false, animation = 0;
  let lastActivated:MatrixCell|undefined;
  let drag: { id: number; anchor: MatrixCell; original: MatrixCell[]; originalAnchor?: MatrixCell; additive: boolean; x: number; y: number; startX: number; startY: number; moved: boolean } | undefined;
  function current(): MatrixCell | undefined {
    const frameId = frameIds.value[s.frame];
    return frameId && s.layer ? { layerId: s.layer, frameId } : undefined;
  }
  function reset() { const cell = current(); selected.value = cell ? [cell] : []; anchor.value = cell; }
  watch(() => s.snapshot?.active_document, () => { stopDrag(); reset(); }, { immediate: true, flush: 'sync' });
  watch([() => s.layer, () => s.frame, () => s.snapshot?.document?.generation], () => {
    const cell=current();
    // Refresh changes the frame index before publishing its new snapshot. Resolve stable IDs afterward.
    if (!internal && !drag && !s.playing && (!cell || !lastActivated || cellKey(cell)!==cellKey(lastActivated))) {reset();lastActivated=cell;}
  }, { flush: 'post' });
  watch([rowIds, frameIds], () => {
    selected.value = selected.value.filter(cell => rowIds.value.includes(cell.layerId) && frameIds.value.includes(cell.frameId));
    if (anchor.value && (!rowIds.value.includes(anchor.value.layerId) || !frameIds.value.includes(anchor.value.frameId))) anchor.value = undefined;
  });
  function activate(cell: MatrixCell) {
    lastActivated=cell;
    internal = true;
    s.layer = cell.layerId;
    editor.select(frameIds.value.indexOf(cell.frameId));
    internal = false;
  }
  function choose(cell: MatrixCell, event?: Pick<MouseEvent, 'shiftKey' | 'ctrlKey' | 'metaKey'>) {
    if (editor.following() || s.working) return;
    const next = selectCell(selected.value, anchor.value, cell, rowIds.value, frameIds.value, { range: event?.shiftKey, additive: event?.ctrlKey || event?.metaKey });
    selected.value = next.selected; anchor.value = next.anchor;
    activate(cell);
  }
  function hit() {
    if (!drag || !root.value) return;
    const viewport = root.value.closest<HTMLElement>('.frame-scroll-area-viewport');
    const box = viewport?.getBoundingClientRect();
    if (!box) return;
    const element = document.elementFromPoint(Math.max(box.left + 2, Math.min(box.right - 12, drag.x)), Math.max(box.top + 2, Math.min(box.bottom - 12, drag.y)))?.closest<HTMLElement>('[data-cell]');
    if (!element || !root.value.contains(element)) return;
    const row = Number(element.dataset.row), column = Number(element.dataset.column);
    const layerId = rowIds.value[row], frameId = frameIds.value[column];
    if (!layerId || !frameId) return;
    const target = { layerId, frameId };
    selected.value = dragCells(drag.original, drag.anchor, target, rowIds.value, frameIds.value, drag.additive);
    activate(target);
  }
  function scroll() {
    if (!drag) return;
    if (drag.moved) {
      const viewport = root.value?.closest<HTMLElement>('.frame-scroll-area-viewport');
      const box = viewport?.getBoundingClientRect();
      if (viewport && box) {
        const dx = drag.x < box.left + 24 ? -12 : drag.x > box.right - 24 ? 12 : 0;
        const dy = drag.y < box.top + 24 ? -10 : drag.y > box.bottom - 24 ? 10 : 0;
        if (dx || dy) { viewport.scrollBy(dx, dy); hit(); }
      }
    }
    animation = requestAnimationFrame(scroll);
  }
  function pointerDown(event: PointerEvent, cell: MatrixCell) {
    if (event.button !== 0 || editor.following() || s.working) return;
    const original = [...selected.value], originalAnchor = anchor.value;
    const start = event.shiftKey && originalAnchor ? originalAnchor : cell;
    choose(cell, event);
    drag = { id: event.pointerId, anchor: start, original, originalAnchor, additive: event.ctrlKey || event.metaKey, x: event.clientX, y: event.clientY, startX: event.clientX, startY: event.clientY, moved: false };
    (event.currentTarget as HTMLElement).focus({ preventScroll: true });
    root.value?.setPointerCapture(event.pointerId);
    event.preventDefault();
    animation = requestAnimationFrame(scroll);
  }
  function pointerMove(event: PointerEvent) {
    if (!drag || event.pointerId !== drag.id) return;
    drag.x = event.clientX; drag.y = event.clientY;
    if (Math.hypot(drag.x - drag.startX, drag.y - drag.startY) > 4) drag.moved = true;
    if (drag.moved) hit();
  }
  function stopDrag(revert = false) {
    const previous = drag; drag = undefined;
    cancelAnimationFrame(animation);
    if (previous && root.value?.hasPointerCapture(previous.id)) root.value.releasePointerCapture(previous.id);
    if (revert && previous) { selected.value = previous.original; anchor.value = previous.originalAnchor; }
    else if (previous?.moved) {
      const row = rowIds.value.indexOf(s.layer ?? '');
      root.value?.querySelector<HTMLElement>(`[data-cell="${row}-${s.frame}"] button`)?.focus({ preventScroll: true });
    }
  }
  function key(event: KeyboardEvent, cell: MatrixCell) {
    if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); stopDrag(true); return; }
    if (event.key === 'Delete' || event.key === 'Backspace') { event.preventDefault(); event.stopPropagation(); clear(); return; }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'a') {
      event.preventDefault(); event.stopPropagation();
      selected.value = rowIds.value.flatMap(layerId => frameIds.value.map(frameId => ({ layerId, frameId })));
      return;
    }
    const delta = { ArrowLeft: [0, -1], ArrowRight: [0, 1], ArrowUp: [-1, 0], ArrowDown: [1, 0] }[event.key];
    if (!delta || editor.following() || s.working) return;
    event.preventDefault(); event.stopPropagation();
    const row = Math.max(0, Math.min(rowIds.value.length - 1, rowIds.value.indexOf(cell.layerId) + delta[0]!));
    const column = Math.max(0, Math.min(frameIds.value.length - 1, frameIds.value.indexOf(cell.frameId) + delta[1]!));
    choose({ layerId: rowIds.value[row]!, frameId: frameIds.value[column]! }, event);
    const button = root.value?.querySelector<HTMLElement>(`[data-cell="${row}-${column}"] button`);
    button?.focus({ preventScroll: true }); button?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }
  onBeforeUnmount(() => stopDrag());
  return { root, selected, selectedKeys, anchor, rowIds, frameIds, choose, activate, pointerDown, pointerMove, stopDrag, key };
}
