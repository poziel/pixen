<script setup lang="ts">
import {
  computed,
  defineComponent,
  h,
  markRaw,
  shallowReactive,
  onBeforeUnmount,
  onMounted,
  provide,
  watch,
} from "vue";
import {
  createFrame,
  FrameShell,
  FrameIcon,
  parseTheme,
  type FrameThemeFormat,
  type FrameMenuItem,
  type PanelAction,
} from "@frame/core";
import { createTauriHost } from "@frame/tauri";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { call } from "./services/backend";
import { createEditor, EditorKey } from "./editor/editor";
import CanvasView from "./editor/canvas/CanvasView.vue";
import EditorDialogActions from "./editor/EditorDialogActions.vue";
import LayersView from "./editor/layers/LayersView.vue";
import ToolsView from './editor/tools/ToolsView.vue';
import TimelineView from "./editor/timeline/TimelineView.vue";
import TimelineControls from "./editor/timeline/TimelineControls.vue";
import AiView from "./views/AiView.vue";
import EditorDialog from "./editor/EditorDialog.vue";
import ReviewView from "./views/ReviewView.vue";
import pixloomThemeText from "../assets/themes/pixloom.yaml?raw";
import appIconUrl from "../assets/pixloom.svg";

const editor = createEditor(),
  s = editor.state;
provide(EditorKey, editor);
const frame = createFrame({
  status: { allowAddSeparator: false, allowMove: false },
  features: {
    secondarySidebar: false,
    secondaryViewBar: false,
    panel: false,
    panelViewBar: false,
    navigationBar: false,
  },
  host: isTauri() ? createTauriHost({ files: false }) : undefined,
  theme: parseTheme(pixloomThemeText, { format: 'yaml' }),
  navigation: false,
  navigationBar: false,
  settings: false,
  notifications: false,
  workspace: {
    allowPaneCreation: false,
    maxPanes: 1,
    newPaneMode: "tabs",
    welcome: { commands: ['pixloom.open', 'pixloom.new'] },
    tabs: { trackChanges: true, allowCreate: true, allowClose: true, allowMove: false, lastTab: 'allow-empty',
      create: () => { s.dialog = 'new'; return undefined; }, beforeClose: closeDocuments,
      contextMenu: (_context, defaults) => defaults.filter(item => item.id !== 'tab.split' && item.id !== 'tab.move'),
    },
  },
  primaryViewBar: { position: "top", size: "compact", display: "icons", allowSizeChange: false, allowDisplayChange: false },
  secondaryViewBar: { position: "hidden" },
  panelViewBar: { visible: false },
  toolbar: {
    contextMenu: false,
    customize: false,
    primary: true,
    bottom: false,
    secondary: false,
    commandCenter: false,
    layoutControls: true,
    placements: { primary: { area: 'statusBar', side: 'left', order: -100 } },
  },
});
Object.assign(frame.layout, {
  secondarySidebarVisible: false,
  secondaryViewBarVisible: false,
  panelVisible: false,
  navigationVisible: false,
  menuBarVisible: true,
  primaryWidth: 300,
});
const icon = (name: string) => markRaw(defineComponent(() => () => h(FrameIcon, { name })));
const appIcon = markRaw(defineComponent(() => () => h('img', { src: appIconUrl, alt: '', width: 20, height: 20 })));
const layerActions = shallowReactive<PanelAction[]>([]);
watch(
  () => s.snapshot?.document?.layers.length,
  (count) =>
    layerActions.splice(
      0,
      layerActions.length,
      ...(count
        ? [
            {
              id: "pixloom.layers.reset",
              label: "Reset layer view",
              icon: icon("refresh"),
              get disabled() {
                return editor.following();
              },
              run: () => {
                s.hidden =
                  s.snapshot?.document?.layers
                    .filter((l) => !l.visible)
                    .map((l) => l.id) ?? [];
                s.isolatedLayer = null;
              },
            },
          ]
        : []),
    ),
  { immediate: true },
);
const previewSpeedIcon = markRaw(defineComponent(() => () => h('svg', {
  viewBox: '0 0 16 16', fill: 'none', stroke: 'currentColor', 'stroke-width': 1,
  'stroke-linecap': 'round', 'aria-hidden': 'true',
}, [h('circle', { cx: 8, cy: 8, r: 5.5 }), h('path', { d: 'M8 4.5V8l2.5 1.5' })])));
const timelineActions: PanelAction[] = [{
  id: "pixloom.timeline.speed", label: "Preview speed", icon: previewSpeedIcon,
  get disabled() { return editor.following(); },
  get menu() { return [0.5, 1, 3, 5, 8, 15, 20].map(fps => ({
    id: String(fps), label: fps + " FPS", checked: s.fps === fps,
  })); },
  run: () => {},
  onSelect: item => { if (!editor.following()) { s.fps = Number(item.id); s.elapsed = 0; } },
}];
for (const [id, label, component, src] of [
  ["tools", "Tools", ToolsView, "edit"],
  ["layers", "Layers", LayersView, "layers"],
  ["timeline", "Timeline", TimelineView, "play"],
  ["ai", "AI chat", AiView, "comment-discussion"],
] as const) {
  frame.registerSidePanel({
    id: `pixloom.${id}`,
    label,
    component: markRaw(component),
    icon: icon(src),
    movable: false,
    draggable: false,
    padded: false,
    actions: id === "layers" ? layerActions : id === "timeline" ? timelineActions : undefined,
    bottom: id === "timeline" ? { component: markRaw(TimelineControls) } : undefined,
  });
}
frame.openSidePanel("pixloom.layers");
let syncingTabs = false, closingTabs = false;
const DocumentCanvas = markRaw(defineComponent({
  props: { documentId: { type: String, required: true } },
  setup: props => () => s.snapshot?.active_document === props.documentId ? h(CanvasView) : null,
}));
const removeTabs = new Map<string, () => void>();
watch(() => s.snapshot, snapshot => {
  if (!snapshot || closingTabs) return;
  syncingTabs = true;
  try {
    const documents = snapshot.documents ?? [];
    if (frame.workspace.options.tabs?.allowCreate === !!snapshot.managed) frame.workspace.configure({ tabs: { allowCreate: !snapshot.managed, allowClose: !snapshot.managed } });
    if (documents.length && frame.workspace.paneMode(frame.workspace.initialPaneId) !== 'tabs') frame.workspace.setPaneMode(frame.workspace.initialPaneId, 'tabs');
    for (const [id, remove] of removeTabs) if (!documents.some(document => document.id === id)) { remove(); removeTabs.delete(id); }
    for (const document of documents) {
      const title = document.path?.split(/[\\/]/).pop() ?? 'Untitled';
      const existing = frame.workspace.tabs.value.find(tab => tab.id === document.id);
      if (!existing) removeTabs.set(document.id, frame.workspace.registerTab(frame.workspace.initialPaneId, {
        id: document.id, title, component: DocumentCanvas, props: { documentId: document.id }, dirty: document.dirty, closable: !snapshot.managed,
      }));
      else if (existing.title !== title || existing.dirty !== document.dirty) frame.workspace.updateTab(document.id, { title, dirty: document.dirty });
    }
    if (snapshot.active_document) frame.workspace.activateTab(snapshot.active_document);
  } finally { syncingTabs = false; }
});
const stopTabEvents = frame.workspace.onDidChange(event => {
  if (syncingTabs || (closingTabs && event.type === 'activate')) return;
  if (event.type === 'activate') {
    const id = event.tabIds[0];
    if (id && id !== s.snapshot?.active_document) void editor.action('activate_document', { documentId: id });
  } else if (event.type === 'close') { closingTabs = false; void editor.refresh(); afterDocumentClose(); }
});
async function closeDocuments(context: { tabIds: readonly string[] }) {
  closingTabs = true;
  try {
    await editor.idle(); s.working = true; await editor.refresh();
    for (const id of context.tabIds) await call('close_document', { documentId: id, discard: true });
    s.working = false; return true;
  } catch (error) { s.working = false; closingTabs = false; s.error = String(error); return false; }
}
const doc = computed(() => s.snapshot?.document);
function afterDocumentClose() {
  if (!frame.workspace.tabs.value.length) frame.primarySidebar.close();
}
const stopCloseCommandHook = frame.commands.hook('frame.workspace.tab.close', afterDocumentClose);
const title = computed(
  () =>
    `${doc.value?.path?.split(/[\\/]/).pop() ?? (doc.value ? "Untitled" : "Pixloom")}${doc.value?.dirty ? " •" : ""}`,
);
const canEdit = () => !s.snapshot?.managed && !s.working;
function command(
  id: string,
  label: string,
  run: () => void | Promise<unknown>,
  shortcut?: string,
  enabled?: () => boolean,
) {
  frame.commands.register({
    id: `pixloom.${id}`,
    feature: 'pixloom.editor',
    label,
    shortcut,
    enabled,
    run: async () => {
      await run();
    },
  });
}
const canNavigateFrames = () => !editor.following() && !!doc.value?.frames.length;
const canPlayFrames = () => canNavigateFrames() && doc.value!.frames.length > 1;
command('first-frame', 'First Frame', () => editor.select(0), 'Ctrl+F5', canNavigateFrames);
command('previous-frame', 'Previous frame', () => editor.step(-1), 'F5', canNavigateFrames);
command('play-reverse', 'Play in reverse', () => editor.play(-1), 'F6', canPlayFrames);
command('play-forward', 'Play forward', () => editor.play(1), 'F7', canPlayFrames);
command('reduce-fps', 'Reduce preview FPS by 1', () => {
  s.fps = Math.max(0.5, s.fps - 1);
  s.elapsed = 0;
}, 'Ctrl+F6', () => canPlayFrames() && s.fps > 0.5);
command('raise-fps', 'Raise preview FPS by 1', () => {
  s.fps += 1;
  s.elapsed = 0;
}, 'Ctrl+F7', canPlayFrames);
command('next-frame', 'Next frame', () => editor.step(1), 'F8', canNavigateFrames);
command('last-frame', 'Last Frame', () => editor.select(doc.value!.frames.length - 1), 'Ctrl+F8', canNavigateFrames);
command(
  "open",
  "Open artwork",
  async () => {
    await editor.action("open_document");
  },
  "Ctrl+O",
  canEdit,
);
command(
  "new",
  "New artwork",
  async () => {
    s.dialog = "new";
  },
  "Ctrl+N",
  canEdit,
);
command(
  "save",
  "Save project",
  () => editor.action("save_document", { saveAs: false }),
  "Ctrl+S",
  () => canEdit() && !!doc.value,
);
command(
  "save-as",
  "Save project as",
  () => editor.action("save_document", { saveAs: true }),
  "Ctrl+Shift+S",
  () => canEdit() && !!doc.value,
);
command(
  "resize",
  "Canvas size",
  () => {
    s.dialog = "resize";
  },
  undefined,
  () => canEdit() && !!doc.value,
);
command(
  "settings",
  "Settings",
  () => {
    s.dialog = "settings";
  },
  "Ctrl+,",
);
command(
  "fit",
  "Fit canvas",
  () => {
    s.zoomReset++;
  },
  "Ctrl+0",
);
command(
  "sheet",
  "Switch sprite / animation view",
  async () => {
    s.playing = false;
    await editor.action("toggle_view");
  },
  undefined,
  () => !!doc.value?.can_toggle_view,
);
for (const id of ["layers", "timeline", "ai"])
  command(id, `Show ${id === "ai" ? "AI chat" : id}`, () =>
    frame.openSidePanel(`pixloom.${id}`),
  );
for (const mode of ["composite", "onion_skin", "frame_comparison"] as const)
  command(
    mode,
    mode === "composite"
      ? "Normal view"
      : mode === "onion_skin"
        ? "Onion skin"
        : "Frame comparison",
    () => {
      s.mode = mode;
      s.playing = false;
      s.sheet = false;
    },
  );
for (const kind of ["analyze", "sheet", "animation"])
  command(
    `review-${kind}`,
    kind === "analyze"
      ? "Analyze current frame"
      : kind === "sheet"
        ? "Review sheet"
        : "Audit animation",
    () => editor.review(kind),
    undefined,
    () => !!doc.value,
  );
for (const mode of ["color", "silhouette", "grayscale"] as const)
  command(`color-${mode}`, mode, () => {
    s.representation = mode;
  });
for (const bg of ["checkerboard", "transparent", "black", "white"])
  command(`background-${bg}`, bg, () => {
    s.background = bg;
  });
command("reset-layers", "Reset layer view", () => {
  s.hidden = doc.value?.layers.filter((l) => !l.visible).map((l) => l.id) ?? [];
  s.isolatedLayer = null;
});
command("follow", "Follow MCP activity", () => {
  s.follow = !s.follow;
});
command("quit", "Quit", async () => {
  await getCurrentWindow().close();
});
const item = (id: string, label: string, checked?: boolean): FrameMenuItem => ({
  id: `menu.${id}`,
  label,
  command: `pixloom.${id}`,
  checked,
});
const menu = computed<FrameMenuItem[]>(() => [
  {
    id: "file",
    label: "File",
    children: [
      item("new", "New artwork…"),
      item("open", "Open artwork…"),
      item("save", "Save project"),
      item("save-as", "Save as…"),
      { id: 'file.close', label: 'Close artwork', command: 'frame.workspace.tab.close', shortcut: 'Ctrl+W' },
      { id: "file.edit.sep", type: "separator" },
      item("resize", "Canvas size…"),
      item("settings", "Settings…"),
      { id: "file.sep", type: "separator" },
      item("quit", "Quit"),
    ],
  },
  {
    id: "view",
    label: "View",
    children: [
      item("fit", "Fit canvas"),
      item("reset-layers", "Reset layer view"),
      item("sheet", "Switch sprite / animation view", s.sheet),
      { id: "view.display.sep", type: "separator" },
      {
        id: "bg",
        label: "Background",
        children: ["checkerboard", "transparent", "black", "white"].map((bg) =>
          item(`background-${bg}`, bg, s.background === bg),
        ),
      },
      {
        id: "color",
        label: "Color mode",
        children: ["color", "silhouette", "grayscale"].map((mode) =>
          item(`color-${mode}`, mode, s.representation === mode),
        ),
      },
      { id: "view.panels.sep", type: "separator" },
      item("layers", "Layers"),
      item("timeline", "Timeline"),
      item("ai", "AI chat"),
      ...(s.snapshot?.managed
        ? [item("follow", "Follow MCP activity", s.follow)]
        : []),
    ],
  },
  {
    id: "inspect",
    label: "Inspect",
    children: [
      item("composite", "Normal view", s.mode === "composite"),
      item("onion_skin", "Onion skin", s.mode === "onion_skin"),
      item(
        "frame_comparison",
        "Frame comparison",
        s.mode === "frame_comparison",
      ),
      { id: "inspect.tools.sep", type: "separator" },
      item("review-analyze", "Analyze current frame"),
      item("review-sheet", "Review sheet"),
      item("review-animation", "Audit animation"),
    ],
  },
]);
watch(
  () => s.dialog,
  (kind) => {
    if (kind)
      frame.openModal({
        id: "pixloom.dialog",
        title:
          kind === "new"
            ? "New artwork"
            : kind === "resize"
              ? "Canvas size"
              : "Settings",
        component: markRaw(EditorDialog),
        props: { kind },
        width: 480,
        height: kind === "settings" ? 440 : kind === "new" ? 360 : 240,
        minHeight: kind === "new" ? 360 : undefined,
        footer: kind === "settings" ? undefined : markRaw(EditorDialogActions),
        footerProps: { kind },
      });
  },
);
watch(
  () => s.showReview,
  (show) => {
    if (show)
      frame.openModal({
        id: "pixloom.review",
        title: "Art review",
        component: markRaw(ReviewView),
        width: 680,
        height: 560,
      });
  },
);
watch(
  () => frame.modals.map((m) => m.id).join(","),
  () => {
    if (!frame.modals.some((m) => m.id === "pixloom.dialog")) {
      s.dialog = "";
      s.previewSize = null;
    }
    if (!frame.modals.some((m) => m.id === "pixloom.review"))
      s.showReview = false;
  },
);
watch(
  () => s.error,
  (error) => {
    if (error) frame.status.show(error, { severity: "error", duration: 8000 });
  },
);
watch(
  () => [s.snapshot?.activity?.label, s.snapshot?.activity?.busy, s.working],
  () =>
    frame.status.set(
      s.working ? "Working…" : s.snapshot?.activity?.label || "Ready",
    ),
);
frame.status.register({
  id: "pixloom.document",
  side: "left",
  label: "Document",
  kind: "text",
  text: "",
  order: 0,
});
frame.status.register({
  id: "pixloom.frame", side: "left", label: "Frame", kind: "text", order: 1,
  available: () => (doc.value?.frames.length ?? 0) > 1,
  get text() { return `Frame ${s.frame + 1} / ${doc.value?.frames.length ?? 0}`; },
});
frame.status.registerSeparator({ id: "pixloom.document-frame", before: "pixloom.frame", allowRemove: true });
frame.status.register({
  id: "pixloom.fps", side: "left", label: "Preview FPS", kind: "text", order: 2,
  available: () => (doc.value?.frames.length ?? 0) > 1,
  get text() { return `${s.fps} FPS`; },
});
const zoomLevels = [25, 50, 100, 200, 400, 800, 1600, 3200];
function changeZoom(options: { scale?: number; factor?: number }) {
  s.zoomRequest = { serial: s.zoomRequest.serial + 1, ...options };
}
for (const percent of zoomLevels)
  command('zoom-' + percent, percent + '%', () => changeZoom({ scale: percent / 100 }));
frame.status.register({
  id: 'pixloom.zoom-out', kind: 'button', label: 'Zoom out', icon: 'remove',
  side: 'right', order: 100, run: () => changeZoom({ factor: 1 / 1.15 }),
});
frame.status.register({
  id: 'pixloom.zoom', kind: 'button', label: 'Canvas zoom', side: 'right', order: 101,
  text: '100%', menu: () => [
    item('fit', 'Fit to content'),
    { id: 'zoom.sep', type: 'separator' },
    ...zoomLevels.map(percent => item('zoom-' + percent, percent + '%', s.canvasZoom === percent)),
  ],
});
frame.status.register({
  id: 'pixloom.zoom-in', kind: 'button', label: 'Zoom in', icon: 'add',
  side: 'right', order: 102, run: () => changeZoom({ factor: 1.15 }),
});
watch(
  () => [doc.value, s.frame, s.canvasWidth, s.canvasHeight, s.canvasZoom],
  () => {
    const d = doc.value;
    frame.status.update("pixloom.document", {
      text: d
        ? `${s.canvasWidth || d.width} × ${s.canvasHeight || d.height}`
        : "No artwork",
    });
    frame.status.update("pixloom.zoom", { text: `${s.canvasZoom}%` });
    for (const id of ["pixloom.zoom-out", "pixloom.zoom", "pixloom.zoom-in"])
      frame.status.setVisible(id, !!d);
  },
  { immediate: true },
);
function key(event: KeyboardEvent) {
  if (
    frame.modals.length ||
    s.dialog ||
    event.ctrlKey ||
    event.metaKey ||
    event.altKey ||
    (event.target as HTMLElement)?.closest(
      "input,textarea,select,button,[contenteditable=true]",
    )
  )
    return;
  if (!doc.value) return;
  if (event.key === "ArrowRight") {
    event.preventDefault();
    editor.step(1);
  } else if (event.key === "ArrowLeft") {
    event.preventDefault();
    editor.step(-1);
  } else if (event.key === "Home") {
    event.preventDefault();
    editor.select(0);
  }
}
let timer: ReturnType<typeof setInterval>,
  animation = 0,
  last = 0,
  unlisten: (() => void) | undefined,
  unclose: (() => void) | undefined;
let exitApproved = false;
let undrop: (() => void) | undefined;
function tick(now: number) {
  if (last) editor.tick(now - last);
  last = now;
  animation = requestAnimationFrame(tick);
}
onMounted(async () => {
  window.addEventListener("keydown", key);
  await editor.refresh();
  // Apply the empty-start preference once, after the first successful snapshot.
  if (s.snapshot && !s.snapshot.document) frame.primarySidebar.close();
  await call("desktop_ready");
  timer = setInterval(editor.refresh, 300);
  animation = requestAnimationFrame(tick);
  try {
    const source = await call<{ text: string; format: FrameThemeFormat }>("theme_config");
    frame.theme.set(parseTheme(source.text, { format: source.format }));
  } catch (error) {
    console.warn('Pixloom: keeping the bundled theme because the selected theme could not be loaded.', error);
  }
  // The welcome screen has no canvas/drop zone yet. Keep opening files there.
  undrop = await getCurrentWindow().onDragDropEvent(async ({ payload }) => {
    if (payload.type === 'drop' && !s.snapshot?.document && !s.snapshot?.managed)
      for (const path of payload.paths) await editor.action('open_document', { path });
  });
  unlisten = await listen("pixloom:update", editor.refresh);
  unclose = await getCurrentWindow().onCloseRequested(async (event) => {
    if (exitApproved || s.snapshot?.managed || !s.snapshot?.documents?.some(document => document.dirty)) return;
    event.preventDefault();
    if (await frame.workspace.confirmUnsavedChanges(s.snapshot.documents.map(document => document.id))) {
      exitApproved = true;
      await getCurrentWindow().close();
    }
  });
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", key);
  clearInterval(timer);
  cancelAnimationFrame(animation);
  unlisten?.();
  unclose?.();
  undrop?.();
  stopTabEvents();
  stopCloseCommandHook();
  frame.dispose();
});
</script>
<template>
  <FrameShell
    :frame="frame"
    :app-icon="appIcon"
    app-name="Pixloom"
    :title-text="title"
    :show-search="false"
    :menu-items="menu"
    />
</template>


