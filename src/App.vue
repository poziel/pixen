<script setup lang="ts">
import {
  computed,
  ref,
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
  type FrameMenuItem,
  type PanelAction,
} from "@frame/core";
import { editorModule } from "@frame/core/modules/editor";
import { createTauriHost } from "@frame/tauri";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { call } from "./services/backend";
import { createEditor, EditorKey } from "./editor/editor";
import { createStructure, StructureKey } from './editor/structure';
import ColorSampler from './editor/tools/ColorSampler.vue';
import CanvasView from "./editor/canvas/CanvasView.vue";
import EditorDialogActions from "./editor/EditorDialogActions.vue";
import LayersView from "./editor/layers/LayersView.vue";
import ToolsView from './editor/tools/ToolsView.vue';
import { tools } from './editor/tools/raster';
import ToolsToolbar from './editor/tools/ToolsToolbar.vue';
import ColorPicker from './editor/tools/ColorPicker.vue';
import TimelineView from "./editor/timeline/TimelineView.vue";
import TimelineControls from "./editor/timeline/TimelineControls.vue";
import {previewSpeedItems} from './editor/timeline/previewSpeed';
import TimelineMatrix from './editor/timeline/TimelineMatrix.vue';
import AiView from "./views/AiView.vue";
import { BrushCleaning } from '@lucide/vue';
import { createArtChatService, ArtChatKey } from './services/ai';
import { createAiSettings, AiSettingsKey } from './services/aiSettings';
import { registerPixenSettings } from './services/settings';
import { createThemes } from './services/themes';
import { createFileSettings } from './services/fileSettings';
import { createStorageMode } from './services/storageMode';
import { configurePreferenceEditorWorkers } from './services/editorWorkers';
import ExportDialogActions from './editor/ExportDialogActions.vue';
import ExportDialog from './editor/ExportDialog.vue';
import EditorDialog from "./editor/EditorDialog.vue";
import ReviewView from "./views/ReviewView.vue";
import pixenThemeText from "../assets/themes/pixen.yaml?raw";
import appIconUrl from "../assets/pixen.svg";

// ============================================================================
// Editor state and shared services
// ============================================================================


// Create the editor and expose its reactive state to child views.
const editor = createEditor(),
  s = editor.state;
provide(EditorKey, editor);

// ============================================================================
// Settings — persist recovery preferences and provide AI settings to child views.
// The Settings command below opens the dialog; its fields live in SettingsView.
// ============================================================================


// Apply recovery changes; Frame restores and persists the interval.
watch(() => s.recoveryInterval, seconds => {
  if (!Number.isInteger(seconds) || seconds < 1 || seconds > 3600) return;
  void call('configure_recovery', { seconds }).catch(error => { s.error = String(error); });
}, { immediate: true });
// Create the persisted AI provider and model preferences.
const aiSettings = createAiSettings();
provide(AiSettingsKey, aiSettings);
// Artwork chat and structure controllers share the active editor state.
const artChat = createArtChatService(aiSettings);
provide(ArtChatKey, artChat);
watch(() => s.snapshot?.documents?.map(document => document.id), ids => {
  if (ids) void artChat.retain(ids);
});

// Create shared layer and frame controllers for panel actions.
const structure = { layer: createStructure(editor, 'layer'), frame: createStructure(editor, 'frame') };
provide(StructureKey, structure);

// ============================================================================
// Application shell — theme, host, workspace, and initial layout defaults.
// ============================================================================

const layoutReady = ref(false);

// Create the shell with the application theme, workspace, and bar options.
const fileSettings = createFileSettings({
  ai: aiSettings,
  legacy: window.localStorage,
  prepare(values) {
    const interval = values['pixen.recoveryInterval'];
    if (interval !== undefined && (!Number.isInteger(interval) || Number(interval) < 1 || Number(interval) > 3600)) throw new Error('Recovery interval must be a whole number from 1 to 3600.');
    const fps = Number(values['pixen.fps']);
    if (values['pixen.fps'] !== undefined && (!Number.isFinite(fps) || fps < 0 || fps > 1000)) throw new Error('Invalid saved preview speed.');
    if (values['pixen.fps'] !== undefined) s.defaults.fps = fps;
    if (typeof values['pixen.theme'] === 'string' && !themes.state.options.some(option => option.value === values['pixen.theme'])) delete values['pixen.theme'];
  },
});
configurePreferenceEditorWorkers();
const frame = createFrame({
  status: { allowAddSeparator: false, allowMove: false },
  features: {
    secondarySidebar: false,
    secondaryViewBar: false,
    panel: true,
    panelViewBar: false,
    navigationBar: false,
  },
  host: isTauri() ? createTauriHost({ files: false }) : undefined,
  theme: parseTheme(pixenThemeText, { format: 'yaml' }),
  navigation: {
    canGoBack: () => !!s.snapshot?.document && !s.working && !s.snapshot?.managed,
    canGoForward: () => !!s.snapshot?.document && !s.working && !s.snapshot?.managed,
    back: () => editor.action('edit_history', { documentId: s.snapshot?.active_document, redo: false }),
    forward: () => editor.action('edit_history', { documentId: s.snapshot?.active_document, redo: true }),
  },
  navigationBar: false,
  settings: { storage: fileSettings.storage },
  keybindings: { storage: {
    read: async () => (await call<string | null>('read_keybindings_file')) ?? undefined,
    write: json => call('write_keybindings_file', { json }),
  } },
  notifications: false,
  workspace: {
    allowPaneCreation: false,
    maxPanes: 1,
    newPaneMode: "tabs",
    welcome: { commands: ['pixen.open', 'pixen.new'] },
    tabs: { showSingleTab: true, trackChanges: true, allowCreate: true, allowClose: true, allowMove: false, lastTab: 'allow-empty',
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
}).use(editorModule);
frame.workspace.setPaneMode(frame.workspace.initialPaneId, 'tabs');

const themes = createThemes(frame);
const storage = createStorageMode({
  idle: () => editor.idle(),
  flush: async () => { await frame.settings.flush(); await frame.keybindings.flush(); },
  stopChats: () => artChat.retain([]),
  restarting: () => { exitApproved = true; },
  resume: () => { exitApproved = false; },
});
const removeApplicationSettings = registerPixenSettings(frame, s, aiSettings, themes, storage);
function blockDuringStorageMigration(event: KeyboardEvent) {
  if (storage.state.busy) { event.preventDefault(); event.stopImmediatePropagation(); }
}

// Set the initial sidebar sizes and panel visibility.
Object.assign(frame.layout, {
  secondarySidebarVisible: false,
  secondaryViewBarVisible: false,
  panelVisible: false,
  navigationVisible: true,
  menuBarVisible: true,
  primaryWidth: 300,
  panelHeight: 360,
});

// Wrap a named Frame icon as a reusable Vue component.
const icon = (name: string) => markRaw(defineComponent(() => () => h(FrameIcon, { name })));
// The logo uses a 16px pixel grid; fractional enlargement makes its edges uneven.
const appIcon = markRaw(defineComponent(() => () => h('img', { src: appIconUrl, alt: '', width: 16, height: 16 })));

// ============================================================================
// Primary bar and panels — Tools, Layers, Timeline, and AI chat.
// Define panel header actions first, then register each bar entry and its content.
// ============================================================================


// Build the add and delete header actions for layers or frames.
function structureActions(kind: 'layer' | 'frame'): PanelAction[] {
  const controller = structure[kind];
  return [
    { id: `pixen.${kind}.add`, label: `Add ${kind}`, icon: icon('add'), get disabled() { return controller.disabled.value; }, run: () => { void controller.add(); } },
    { id: `pixen.${kind}.delete`, label: `Delete ${kind}`, icon: icon('trash'), get disabled() { return controller.disabled.value || controller.items.value.length < 2; }, run: () => controller.requestDelete() },
  ];
}

// Add a reset action that restores the document’s layer visibility.
const layerActions: PanelAction[] = [...structureActions('layer'), {
  id: 'pixen.layers.reset', label: 'Reset layer view', icon: icon('refresh'),
  get disabled() { return editor.following(); },
  run: () => { s.hidden = s.snapshot?.document?.layers.filter(layer => !layer.visible).map(layer => layer.id) ?? []; s.isolatedLayer = null; },
}];

// Draw the clock icon used by the preview speed menu.
const previewSpeedIcon = markRaw(defineComponent(() => () => h('svg', {
  viewBox: '0 0 16 16', fill: 'none', stroke: 'currentColor', 'stroke-width': 1,
  'stroke-linecap': 'round', 'aria-hidden': 'true',
}, [h('circle', { cx: 8, cy: 8, r: 5.5 }), h('path', { d: 'M8 4.5V8l2.5 1.5' })])));

// Combine frame creation/deletion with the preview speed selector.
const timelineActions: PanelAction[] = [...structureActions('frame'), {
  id: "pixen.timeline.speed", label: "Preview speed", icon: previewSpeedIcon,
  get disabled() { return editor.following(); },
  get menu() { return previewSpeedItems(s.fps); },
  run: () => {},
  onSelect: item => { if (!editor.following()) { s.fps = Number(item.id); s.elapsed = 0; } },
}];

// Clear the active artwork’s chat when it is available for interaction.
const aiActions: PanelAction[] = [{
  id: 'pixen.ai.clear', label: 'Clear chat',
  icon: markRaw(defineComponent(() => () => h(BrushCleaning, { size: 16, strokeWidth: 1.5, 'aria-hidden': 'true' }))),
  get disabled() {
    const id = s.snapshot?.active_document;
    return !id || !!s.snapshot?.managed || artChat.entry(id).connecting;
  },
  run: () => { const id = s.snapshot?.active_document; if (id) void artChat.close(id); },
}];

// Register the Tools sidebar entry and its color and mouse-tool sections.
frame.registerSidePanel({
  id: 'pixen.tools', label: 'Tools', icon: icon('edit'), movable: false, draggable: false,
  children: [

    // Choose the foreground and background drawing colors.
    { id: 'pixen.colors', label: 'Color picker', component: markRaw(ColorPicker), padded: false },

    // Configure the tool assigned to the left mouse button.
    { id: 'pixen.left-tool', get label() { return `Left tools · ${tools.find(tool => tool.id === s.tools.left)?.label ?? s.tools.left}`; }, titleColor: '#28a9f4', component: markRaw(ToolsView), props: { slot: 'left' }, collapsed: true, padded: false },

    // Configure the tool assigned to the right mouse button.
    { id: 'pixen.right-tool', get label() { return `Right tools · ${tools.find(tool => tool.id === s.tools.right)?.label ?? s.tools.right}`; }, titleColor: '#ff8b36', component: markRaw(ToolsView), props: { slot: 'right' }, collapsed: true, padded: false },
  ],
});

// Register Layers, Timeline, and AI chat in their primary-bar order.
for (const [id, label, component, src] of [
  ["layers", "Layers", LayersView, "layers"],
  ["timeline", "Timeline", TimelineView, "play"],
  ["ai", "AI chat", AiView, "comment-discussion"],
] as const) {
  frame.registerSidePanel({
    id: `pixen.${id}`,
    label,
    component: markRaw(component),
    icon: icon(src),
    movable: false,
    draggable: false,
    padded: false,
    actions: id === "layers" ? layerActions : id === "timeline" ? timelineActions : aiActions,
    bottom: id === "timeline" ? { component: markRaw(TimelineControls) } : undefined,
  });
}
// Bottom timeline panel and floating drawing toolbar.
frame.registerBottomTab({ id: 'pixen.matrix', label: 'Timeline', component: markRaw(TimelineMatrix), padded: false, showHeader: false, movable: false, draggable: false });
const matrixVisible = computed(() => frame.layout.panelVisible);
const matrixSidebarVisibility = new Map<string, boolean>();
watch(matrixVisible, visible => {
  if (visible && ['pixen.layers', 'pixen.timeline'].includes(frame.state.activeSidePanel)) frame.primarySidebar.select('pixen.tools');
  for (const id of ['pixen.layers', 'pixen.timeline']) {
    if (visible) matrixSidebarVisibility.set(id, frame.primaryViewBar.isVisible(id));
    frame.primaryViewBar.setVisible(id, visible ? false : (matrixSidebarVisibility.get(id) ?? true));
  }
}, { flush: 'sync' });
function toggleMatrix() {
  if (!s.snapshot?.document) return;
  if (matrixVisible.value) frame.closeBottomPanel();
  else frame.openBottomPanel('pixen.matrix');
}
watch(() => !!s.snapshot?.document, hasArtwork => { if (!hasArtwork) frame.closeBottomPanel(); });

// Register the floating drawing toolbar along the bottom of the canvas.
frame.docks.register({
  id: 'pixen.drawing-tools',
  label: 'Drawing tools',
  component: markRaw(ToolsToolbar),
  anchor: 'workspace',
  edge: 'bottom',
  placement: 'overlay',
  appearance: 'none',
  visible: false,
});
// Select Tools without opening the sidebar during initialization.
frame.state.activeSidePanel = 'pixen.tools';
watch(() => !!s.snapshot?.document, (hasArtwork, hadArtwork) => {
  frame.docks.setVisible('pixen.drawing-tools', hasArtwork);
  // An existing layout wins; otherwise reopen the last selected sidebar tab.
  if (hasArtwork && !hadArtwork && !frame.layout.primarySidebarVisible && !matrixVisible.value) frame.openSidePanel();
  else if (!hasArtwork) frame.layout.primarySidebarVisible = false;
}, { immediate: true, flush: 'sync' });

// ============================================================================
// Document workspace — synchronize tabs, active artwork, and close behavior.
// ============================================================================

let syncingTabs = false, closingTabs = false;

// Render a document’s canvas only while that document is active.
const DocumentCanvas = markRaw(defineComponent({
  props: { documentId: { type: String, required: true } },
  setup: props => () => s.snapshot?.active_document === props.documentId ? h(CanvasView) : null,
}));
const removeTabs = new Map<string, () => void>();
watch(() => s.snapshot, (snapshot, previous) => {
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
    const selected = frame.workspace.activeTabs.value[frame.workspace.initialPaneId];
    if (snapshot.active_document && (!selected || removeTabs.has(selected)) &&
        (!selected || snapshot.active_document !== previous?.active_document)) frame.workspace.activateTab(snapshot.active_document);
  } finally { syncingTabs = false; }
  afterDocumentClose();
});

// Forward tab activation and closure to the editor backend.
const stopTabEvents = frame.workspace.onDidChange(event => {
  if (syncingTabs || (closingTabs && event.type === 'activate') || event.tabIds.every(id => !removeTabs.has(id))) return;
  if (event.type === 'activate') {
    const id = event.tabIds[0];
    if (id && s.snapshot?.documents?.some(document => document.id === id) && id !== s.snapshot?.active_document) void editor.action('activate_document', { documentId: id });
  } else if (event.type === 'close') { closingTabs = false; void editor.refresh(); afterDocumentClose(); }
});
async function closeDocuments(context: { tabIds: readonly string[] }) {
  const documentIds = context.tabIds.filter(id => removeTabs.has(id));
  if (!documentIds.length) return true;
  closingTabs = true;
  try {
    await editor.idle(); s.working = true; await editor.refresh();
    for (const id of documentIds) await call('close_document', { documentId: id, discard: true });
    s.working = false; return true;
  } catch (error) { s.working = false; closingTabs = false; s.error = String(error); return false; }
}
const doc = computed(() => s.snapshot?.document);
function afterDocumentClose() {
  if (!s.snapshot?.documents?.length) frame.primarySidebar.close();
}

// Close the sidebar after the last document tab is closed.
const stopCloseCommandHook = frame.commands.hook('frame.workspace.tab.close', afterDocumentClose);
const title = computed(
  () =>
    `${doc.value?.path?.split(/[\\/]/).pop() ?? (doc.value ? "Untitled" : "Pixen")}${doc.value?.dirty ? " •" : ""}`,
);
// Shared zoom values used by commands and the status-bar menu.
const zoomLevels = [25, 50, 100, 200, 400, 800, 1600, 3200];
function changeZoom(options: { scale?: number; factor?: number }) {
  s.zoomRequest = { serial: s.zoomRequest.serial + 1, ...options };
}

// ============================================================================
// Commands — register application actions and their keyboard shortcuts here.
// ============================================================================

const canEdit = () => !s.snapshot?.managed && !s.working;

// Register a Pixen command and restore canvas focus after it completes.
function command(
  id: string,
  label: string,
  run: () => void | Promise<unknown>,
  shortcut?: string,
  enabled?: () => boolean,
) {
  frame.commands.register({
    id: `pixen.${id}`,
    feature: 'pixen.editor',
    label,
    shortcut,
    enabled,
    run: async () => {
      await run();
      s.canvasFocus++;
    },
  });
}
const canNavigateFrames = () => !editor.following() && !!doc.value?.frames.length;
const canPlayFrames = () => canNavigateFrames() && doc.value!.frames.length > 1;

// Select the first animation frame.
command('first-frame', 'First Frame', () => editor.select(0), 'Ctrl+F5', canNavigateFrames);

// Step backward by one frame.
command('previous-frame', 'Previous frame', () => editor.step(-1), 'F5', canNavigateFrames);

// Play the animation backward.
command('play-reverse', 'Play in reverse', () => editor.play(-1), 'F6', canPlayFrames);

// Play the animation forward.
command('play-forward', 'Play forward', () => editor.play(1), 'F7', canPlayFrames);

// Slow the preview down, keeping at least 0.5 FPS.
command('reduce-fps', 'Reduce preview FPS by 1', () => {
  s.fps = Math.max(0.5, s.fps - 1);
  s.elapsed = 0;
}, 'Ctrl+F6', () => canPlayFrames() && s.fps > 0.5);

// Speed the preview up by one FPS.
command('raise-fps', 'Raise preview FPS by 1', () => {
  s.fps += 1;
  s.elapsed = 0;
}, 'Ctrl+F7', canPlayFrames);

// Step forward by one frame.
command('next-frame', 'Next frame', () => editor.step(1), 'F8', canNavigateFrames);

// Select the last animation frame.
command('last-frame', 'Last Frame', () => editor.select(doc.value!.frames.length - 1), 'Ctrl+F8', canNavigateFrames);

// Open an existing artwork through the desktop file picker.
command(
  "open",
  "Open artwork",
  async () => {
    await editor.action("open_document");
  },
  "Ctrl+O",
  canEdit,
);

// Open the dialog for creating a new artwork.
command(
  "new",
  "New artwork",
  async () => {
    s.dialog = "new";
  },
  "Ctrl+N",
  canEdit,
);

// Save the active project to its current path.
command(
  "save",
  "Save project",
  () => editor.action("save_document", { saveAs: false }),
  "Ctrl+S",
  () => canEdit() && !s.saving && !!doc.value,
);

// Save the active project to a newly selected path.
command(
  "save-as",
  "Save project as",
  () => editor.action("save_document", { saveAs: true }),
  "Ctrl+Shift+S",
  () => canEdit() && !s.saving && !!doc.value,
);

// Open the canvas size dialog for the active artwork.
command(
  "resize",
  "Canvas size",
  () => {
    s.dialog = "resize";
  },
  undefined,
  () => canEdit() && !!doc.value,
);

// Open application settings.
command(
  "settings",
  "Settings",
  () => {
    frame.openSettings();
  },
  "Ctrl+,",
);

// Fit the artwork into the available canvas area.
command(
  "fit",
  "Fit canvas",
  () => {
    s.zoomReset++;
  },
  "Ctrl+0",
);

// Stop playback and switch between sprite sheet and animation views.
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

// Keep sidebar shortcuts in visual tab order: Tools, Layers, Timeline, AI chat.
for (const [index,id] of ["tools", "layers", "timeline", "ai"].entries())
  command(id, `Show ${id === "ai" ? "AI chat" : id}`, () => {
    if (id === 'layers' || id === 'timeline') frame.closeBottomPanel();
    frame.openSidePanel(`pixen.${id}`);
  },
    `Ctrl+${index+1}`,
  );

// Register the normal, onion skin, and frame comparison inspection modes.
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

// Register the current frame, sprite sheet, and animation review actions.
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

// Register color, silhouette, and grayscale preview representations.
for (const mode of ["color", "silhouette", "grayscale"] as const)
  command(`color-${mode}`, mode[0].toUpperCase() + mode.slice(1), () => {
    s.representation = mode;
  });

// Register each available canvas background.
for (const bg of ["checkerboard", "transparent", "black", "white"])
  command(`background-${bg}`, bg[0].toUpperCase() + bg.slice(1), () => {
    s.background = bg;
  });

// Restore document layer visibility and clear the isolated layer.
command("reset-layers", "Reset layer view", () => {
  s.hidden = doc.value?.layers.filter((l) => !l.visible).map((l) => l.id) ?? [];
  s.isolatedLayer = null;
});

// Toggle whether the editor follows MCP activity.
command("follow", "Follow MCP activity", () => {
  s.follow = !s.follow;
});

// Request window closure through the existing unsaved changes flow.
command("quit", "Quit", async () => {
  await getCurrentWindow().close();
});

// Export the active artwork with the selected output format and options.
command('export', 'Export artwork', () => { s.error = ''; s.playing = false; s.dialog = 'export'; }, 'Ctrl+Shift+E', () => canEdit() && !!doc.value);

// Register one command for each fixed canvas zoom percentage.
for (const percent of zoomLevels)
  command('zoom-' + percent, percent + '%', () => changeZoom({ scale: percent / 100 }));


// ============================================================================
// Menu items — compose the File, View, and Inspect menus from command IDs.
// ============================================================================


// Create a menu entry linked to a Pixen command, with an optional check mark.
const item = (id: string, label: string, checked?: boolean): FrameMenuItem => ({
  id: `menu.${id}`,
  label,
  command: `pixen.${id}`,
  checked,
});

// Build reactive application menus so checked states follow editor state.
const menu = computed<FrameMenuItem[]>(() => [

  // Group artwork creation, opening, saving, export, and application actions.
  {
    id: "file",
    label: "File",
    children: [

      // Create a new artwork.
      item("new", "New artwork…"),

      // Open artwork from disk.
      item("open", "Open artwork…"),

      // Save the active project.
      item("save", "Save project"),

      // Choose a new project path.
      item("save-as", "Save as…"),

      // Export artwork to another format.
      item('export', 'Export…'),

      // Close the active artwork through Frame’s unsaved changes flow.
      { id: 'file.close', label: 'Close artwork', command: 'frame.workspace.tab.close', shortcut: 'Ctrl+W' },

      // Separate file operations from canvas and application settings.
      { id: "file.edit.sep", type: "separator" },

      // Change canvas dimensions.
      item("resize", "Canvas size…"),

      // Open application preferences.
      item("settings", "Settings…"),

      // Separate settings from the Quit action.
      { id: "file.sep", type: "separator" },

      // Request application closure.
      item("quit", "Quit"),
    ],
  },

  // Group canvas presentation and sidebar visibility choices.
  {
    id: "view",
    label: "View",
    children: [

      // Fit the artwork into the canvas viewport.
      item("fit", "Fit canvas"),

      // Restore the document’s layer visibility.
      item("reset-layers", "Reset layer view"),

      // Switch sprite sheet and animation views.
      item("sheet", "Switch sprite / animation view", s.sheet),

      // Separate canvas actions from display choices.
      { id: "view.display.sep", type: "separator" },

      // Offer background choices and mark the active background.
      {
        id: "bg",
        label: "Background",
        children: ["checkerboard", "transparent", "black", "white"].map((bg) =>
          item(`background-${bg}`, bg[0].toUpperCase() + bg.slice(1), s.background === bg),
        ),
      },

      // Offer color representations and mark the active representation.
      {
        id: "color",
        label: "Color mode",
        children: ["color", "silhouette", "grayscale"].map((mode) =>
          item(`color-${mode}`, mode[0].toUpperCase() + mode.slice(1), s.representation === mode),
        ),
      },

      // Separate display choices from sidebar selectors.
      { id: "view.panels.sep", type: "separator" },

      // Open drawing tools.
      { ...item("tools", "Tools"), shortcut:"Ctrl+1" },

      // Open the layer list.
      { ...item("layers", "Layers"), shortcut:"Ctrl+2" },

      // Open the timeline sidebar.
      { ...item("timeline", "Timeline"), shortcut:"Ctrl+3" },

      // Open the artwork chat.
      { ...item("ai", "AI chat"), shortcut:"Ctrl+4" },

      // Offer MCP following only for managed artwork sessions.
      ...(s.snapshot?.managed
        ? [item("follow", "Follow MCP activity", s.follow)]
        : []),
    ],
  },

  // Group inspection modes and artwork review actions.
  {
    id: "inspect",
    label: "Inspect",
    children: [

      // Show the normal artwork preview.
      item("composite", "Normal view", s.mode === "composite"),

      // Show neighboring frames as onion-skin guides.
      item("onion_skin", "Onion skin", s.mode === "onion_skin"),

      // Compare the current frame with another frame.
      item(
        "frame_comparison",
        "Frame comparison",
        s.mode === "frame_comparison",
      ),

      // Separate inspection modes from review actions.
      { id: "inspect.tools.sep", type: "separator" },

      // Analyze the active frame.
      item("review-analyze", "Analyze current frame"),

      // Review the sprite sheet.
      item("review-sheet", "Review sheet"),

      // Audit the animation.
      item("review-animation", "Audit animation"),
    ],
  },
]);

// ============================================================================
// Dialogs — open editor/review modals and synchronize their dismissal.
// ============================================================================

watch(
  () => s.dialog,
  (kind) => {
    if (kind)
      frame.openModal({
        id: "pixen.dialog",
        title:
          kind === "new"
            ? "New artwork"
            : kind === "resize"
              ? "Canvas size"
              : kind === "export" ? "Export artwork" : "Settings",
        component: kind === "export" ? markRaw(ExportDialog) : markRaw(EditorDialog),
        props: { kind },
        ...(kind === 'resize' ? { draggable: false, resizable: false, maximizable: false, showCloseButton: false } : {}),
        width: kind === "export" ? 640 : kind === "settings" ? 960 : 480,
        height: kind === "export" ? 720 : kind === "settings" ? 660 : kind === "new" ? 360 : 240,
        minHeight: kind === "new" ? 360 : undefined,
        footer: kind === "settings" ? undefined : kind === "export" ? markRaw(ExportDialogActions) : markRaw(EditorDialogActions),
        footerProps: { kind },
      });
  },
);
watch(
  () => s.showReview,
  (show) => {
    if (show)
      frame.openModal({
        id: "pixen.review",
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
    if (!frame.modals.some((m) => m.id === "pixen.dialog")) {
      s.dialog = "";
      s.previewSize = null;
    }
    if (!frame.modals.some((m) => m.id === "pixen.review"))
      s.showReview = false;
  },
);

// ============================================================================
// Status bar — notifications, document details, zoom, and layout controls.
// ============================================================================

watch(() => s.saveNotice, () => frame.status.show('Project saved', { duration: 2500 }));
let recoveryNotice = '';
watch(() => [s.snapshot?.active_document, s.snapshot?.recovery] as const, ([id, recovery]) => {
  if (!recovery) return;
  const notice = `${id}:${recovery.revision}:${recovery.saved_at}:${recovery.error}`;
  if (notice === recoveryNotice) return;
  recoveryNotice = notice;
  if (recovery.error) frame.status.show(recovery.error, { severity: 'error', duration: 8000 });
  else if (!recovery.writing && recovery.saved_at) frame.status.show('Recovery file updated', { duration: 2500 });
});

// A cel grid with an up/down arrow distinguishes the matrix and its state.
const matrixIcon = (visible: boolean) => markRaw(defineComponent(() => () => h('svg', {
  viewBox: '0 0 16 16', fill: 'none', stroke: 'currentColor', 'stroke-width': 1,
  'aria-hidden': 'true',
}, [h('rect', { x: 1.5, y: 6.5, width: 13, height: 8, rx: 1 }),
  h('path', { d: 'M1.5 10.5h13M5.5 6.5v8M10 6.5v8' }),
  h('path', { d: visible ? 'M5.5 1.5 8 4l2.5-2.5' : 'M5.5 4 8 1.5 10.5 4' })])));
const showMatrixIcon = matrixIcon(false), hideMatrixIcon = matrixIcon(true);

// Show or hide the bottom timeline matrix.
frame.status.register({
  id: 'pixen.timeline-layout', kind: 'button', side: 'left', order: -99,
  label: 'Toggle timeline matrix', controls: 'bottom-panel',
  get icon() { return matrixVisible.value ? hideMatrixIcon : showMatrixIcon; },
  get expanded() { return matrixVisible.value; },
  get tooltip() { return matrixVisible.value ? 'Hide timeline matrix' : 'Show timeline matrix'; },
  get disabled() { return !s.snapshot?.document; },
  run: toggleMatrix,
});

frame.status.registerSeparator({ id: 'pixen.after-timeline-matrix', before: 'pixen.document' });
frame.status.registerSeparator({ id: 'pixen.before-manage', before: 'frame.manage' });

// A floating tool strip with a pencil stays distinct from shell bar icons.
const drawingToolbarIcon = (visible: boolean) => markRaw(defineComponent(() => () => h('svg', {
  viewBox: '0 0 16 16', fill: 'none', stroke: 'currentColor', 'stroke-width': 1,
  'stroke-linecap': 'round', 'stroke-linejoin': 'round', 'aria-hidden': 'true',
}, [h('rect', { x: 1.5, y: 9.5, width: 13, height: 5, rx: 2 }),
  h('path', { d: 'M4 11.5v1M7 11.5v1M10 11.5v1M13 11.5v1M5 7l1-3 3-3 2 2-3 3-3 1Z' }),
  ...(visible ? [] : [h('path', { d: 'M1.5 1.5l13 13' })])])));
const showDrawingToolbarIcon = drawingToolbarIcon(false), hideDrawingToolbarIcon = drawingToolbarIcon(true);

// Show or hide the floating drawing toolbar.
frame.status.register({
  id: 'pixen.toggle-drawing-tools', kind: 'button', side: 'right', order: 1000,
  label: 'Toggle drawing toolbar',
  get icon() { return frame.docks.get('pixen.drawing-tools')?.visible ? hideDrawingToolbarIcon : showDrawingToolbarIcon; },
  get tooltip() { return frame.docks.get('pixen.drawing-tools')?.visible ? 'Hide drawing toolbar' : 'Show drawing toolbar'; },
  get expanded() { return !!frame.docks.get('pixen.drawing-tools')?.visible; },
  run: () => { frame.docks.toggle('pixen.drawing-tools'); },
});
watch(
  () => s.error,
  (error) => {
    if (error) frame.status.show(error, { severity: "error", duration: 8000 });
  },
);
watch(
  () => [s.working, s.saving, s.snapshot?.recovery?.writing],
  () =>
    frame.status.set(
      s.working ? "Working…" : s.saving ? "Saving project…" : s.snapshot?.recovery?.writing ? "Updating recovery file…" : "Ready",
    ),
);

// Display the active canvas dimensions or the empty-workspace message.
frame.status.register({
  id: "pixen.document",
  side: "left",
  label: "Document",
  kind: "text",
  text: "",
  order: 0,
});

// Display the current frame number for animations.
frame.status.register({
  id: "pixen.frame",
  side: "left",
  label: "Frame",
  kind: "text",
  order: 1,
  available: () => (doc.value?.frames.length ?? 0) > 1,
  get text() { return `Frame ${s.frame + 1} / ${doc.value?.frames.length ?? 0}`; },
});

// Separate document dimensions from the frame counter.
frame.status.registerSeparator({
  id: "pixen.document-frame",
  before: "pixen.frame"
});

// Display preview FPS or indicate that document frame timings are in use.
frame.status.register({
  id: "pixen.fps",
  side: "right",
  label: "Preview FPS", kind: "text",
  order: 2,
  available: () => (doc.value?.frames.length ?? 0) > 1,
  get text() { return s.fps>0?`${s.fps} FPS`:'Frame timings'; },
});

// Separate the right-side details from the zoom controls.
frame.status.registerSeparator({
  id: "pixen.document-frame2",
  before: "pixen.zoom-out"
});

// Reduce canvas magnification by one step.
frame.status.register({
  id: 'pixen.zoom-out',
  kind: 'button',
  label: 'Zoom out',
  icon: 'remove',
  side: 'right',
  order: 100,
  run: () => changeZoom({ factor: 1 / 1.15 }),
});

// Display canvas magnification and offer fit and fixed-zoom choices.
frame.status.register({
  id: 'pixen.zoom',
  kind: 'button',
  label: 'Canvas zoom',
  side: 'right',
  order: 101,
  text: '100%',
  menu: () => [

    // Fit the artwork into the canvas viewport.
    item('fit', 'Fit to content'),

    // Separate fitting the canvas from fixed zoom percentages.
    { id: 'zoom.sep', type: 'separator' },

    // Offer each fixed zoom level and check the active percentage.
    ...zoomLevels.map(percent => item('zoom-' + percent, percent + '%', s.canvasZoom === percent)),
  ],
});

// Increase canvas magnification by one step.
frame.status.register({
  id: 'pixen.zoom-in', kind: 'button', label: 'Zoom in', icon: 'add',
  side: 'right', order: 102, run: () => changeZoom({ factor: 1.15 }),
});
watch(
  () => [doc.value, s.frame, s.canvasWidth, s.canvasHeight, s.canvasZoom],
  () => {
    const d = doc.value;
    frame.status.update("pixen.document", {
      text: d
        ? `${s.canvasWidth || d.width} × ${s.canvasHeight || d.height}`
        : "No artwork",
    });
    frame.status.update("pixen.zoom", { text: `${s.canvasZoom}%` });
    for (const id of ["pixen.zoom-out", "pixen.zoom", "pixen.zoom-in"])
      frame.status.setVisible(id, !!d);
  },
  { immediate: true },
);

// ============================================================================
// Keyboard input and lifecycle — desktop events, refresh, playback, and cleanup.
// ============================================================================


// Handle plain frame-navigation keys outside dialogs and editable controls.
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

  // Right Arrow advances to the next frame.
  if (event.key === "ArrowRight") {
    event.preventDefault();
    editor.step(1);
  } else if (event.key === "ArrowLeft") {
    // Left Arrow returns to the previous frame.
    event.preventDefault();
    editor.step(-1);
  } else if (event.key === "Home") {
    // Home selects the first frame.
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
  window.addEventListener("keydown", blockDuringStorageMigration, true);
  await storage.load();
  let preferencesReady = true;
  await themes.load();
  fileSettings.start();
  try { await frame.settings.load(); await frame.settings.flush(); }
  catch (error) { preferencesReady = false; frame.status.show(`Settings could not be loaded or saved: ${error}`, { severity: 'error', duration: 10000 }); }
  try { await frame.keybindings.load(); await frame.keybindings.flush(); }
  catch (error) { preferencesReady = false; frame.status.show(`Keyboard shortcuts could not be loaded or saved: ${error}`, { severity: 'error', duration: 10000 }); }
  await editor.refresh();
  timer = setInterval(editor.refresh, 300);
  animation = requestAnimationFrame(tick);
  layoutReady.value = true;
  // The welcome screen has no canvas/drop zone yet. Keep opening files there.
  undrop = await getCurrentWindow().onDragDropEvent(async ({ payload }) => {
    if (payload.type === 'drop' && !s.snapshot?.document && !s.snapshot?.managed)
      for (const path of payload.paths) await editor.action('open_document', { path });
  });
  unlisten = await listen("pixen:update", editor.refresh);
  unclose = await getCurrentWindow().onCloseRequested(async (event) => {
    if (exitApproved) return;
    event.preventDefault();
    if (storage.state.busy) return;
    const dirtyTabs = frame.workspace.tabs.value.filter(tab => tab.dirty).map(tab => tab.id);
    if (dirtyTabs.length && !await frame.workspace.confirmUnsavedChanges(dirtyTabs)) return;
    try {
      if (frame.settings.loaded.value) await frame.settings.flush();
      if (frame.keybindings.loaded.value) await frame.keybindings.flush();
      exitApproved = true;
      await getCurrentWindow().close();
    } catch (error) {
      frame.status.show(`Settings could not be saved: ${error}`, { severity: 'error', duration: 10000 });
    }
  });
  await call('desktop_ready', { preferencesReady });
});
onBeforeUnmount(() => {
  void artChat.dispose();
  removeApplicationSettings();
  aiSettings.dispose();
  window.removeEventListener("keydown", key);
  window.removeEventListener("keydown", blockDuringStorageMigration, true);
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
    v-if="layoutReady"
    :frame="frame"
    :app-icon="appIcon"
    app-name="Pixen"
    :title-text="title"
    :show-search="false"
    :menu-items="menu"
    />
  <ColorSampler />
  <div v-if="storage.state.busy" class="storage-migration-progress" role="alert" aria-live="assertive">Preparing storage transfer… Pixen will restart automatically.</div>
</template>


<style scoped>
.storage-migration-progress { position:fixed;inset:0;z-index:2147483647;display:grid;place-items:center;background:var(--frame-bg);color:var(--frame-text);padding:32px; }
</style>
