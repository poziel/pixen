<script setup lang="ts">
import {
  computed,
  defineComponent,
  h,
  markRaw,
  onBeforeUnmount,
  onMounted,
  provide,
  watch,
} from "vue";
import { createFrame, FrameShell, type FrameMenuItem } from "@frame/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { call, native } from "./backend";
import { createEditor, EditorKey } from "./editor";
import CanvasView from "./CanvasView.vue";
import LayersView from "./LayersView.vue";
import TimelineView from "./TimelineView.vue";
import AiView from "./AiView.vue";
import EditorDialog from "./EditorDialog.vue";
import ReviewView from "./ReviewView.vue";
import {
  applyTheme,
  defaultTheme,
  pixloomTheme,
  type ThemeConfig,
} from "./theme";
import layersIcon from "../assets/icons/layers.svg";
import playIcon from "../assets/icons/play.svg";
import chatIcon from "../assets/icons/chat.svg";

const editor = createEditor(),
  s = editor.state;
provide(EditorKey, editor);
const frame = createFrame({
  theme: pixloomTheme(defaultTheme),
  navigation: false,
  navigationBar: false,
  settings: false,
  notifications: false,
  workspace: {
    allowPaneCreation: false,
    maxPanes: 1,
    newPaneMode: "plain",
    tabs: { allowCreate: false, allowClose: false, allowMove: false },
  },
  secondaryViewBar: { position: "hidden" },
  panelViewBar: { visible: false },
  toolbar: {
    customize: false,
    primary: false,
    bottom: false,
    secondary: false,
    commandCenter: false,
    layoutControls: false,
  },
});
Object.assign(frame.layout, {
  secondarySidebarVisible: false,
  secondaryViewBarVisible: false,
  panelVisible: false,
  navigationVisible: false,
  menuBarVisible: true,
  primaryWidth: 260,
});
// Keep unavailable regions closed even when Frame's built-in layout shortcuts run.
watch(
  () => [
    frame.layout.secondarySidebarVisible,
    frame.layout.secondaryViewBarVisible,
    frame.layout.panelVisible,
    frame.layout.panelViewBarVisible,
    frame.layout.navigationVisible,
  ],
  () => {
    frame.closeSecondaryPanel();
    frame.closeBottomPanel();
    frame.navigationBar.setVisible(false);
    frame.secondaryViewBar.setPosition("hidden");
    frame.panelViewBar.setBarVisible(false);
  },
  { flush: "sync" },
);
const icon = (src: string) =>
  markRaw(
    defineComponent(
      () => () => h("img", { src, alt: "", style: "width:20px;height:20px" }),
    ),
  );
for (const [id, label, component, src] of [
  ["layers", "Layers", LayersView, layersIcon],
  ["timeline", "Timeline", TimelineView, playIcon],
  ["ai", "AI chat", AiView, chatIcon],
] as const) {
  frame.registerSidePanel({
    id: `pixloom.${id}`,
    label,
    component: markRaw(component),
    icon: icon(src),
    movable: false,
    draggable: false,
    padded: false,
  });
}
frame.openSidePanel("pixloom.layers");
const doc = computed(() => s.snapshot?.document);
const title = computed(
  () =>
    `${doc.value?.path?.split(/[\\/]/).pop() ?? (doc.value ? "Untitled" : "Pixloom")}${doc.value?.dirty ? " •" : ""}`,
);
const canEdit = () => !s.snapshot?.managed && !s.working;
async function confirmReplace() {
  if (!doc.value?.dirty) return true;
  return window.confirm("Discard unsaved artwork and continue?");
}
function command(
  id: string,
  label: string,
  run: () => void | Promise<unknown>,
  shortcut?: string,
  enabled?: () => boolean,
) {
  frame.registerCommand({
    id: `pixloom.${id}`,
    label,
    shortcut,
    enabled,
    run: async () => {
      await run();
    },
  });
}
command(
  "open",
  "Open artwork",
  async () => {
    if (await confirmReplace()) await editor.action("open_document");
  },
  "Ctrl+O",
  canEdit,
);
command(
  "new",
  "New artwork",
  async () => {
    if (await confirmReplace()) s.dialog = "new";
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
  if (native) await getCurrentWindow().close();
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
      item("resize", "Canvas size…"),
      { id: "file.sep", type: "separator" },
      item("quit", "Quit"),
    ],
  },
  {
    id: "view",
    label: "View",
    children: [
      item("layers", "Layers"),
      item("timeline", "Timeline"),
      item("ai", "AI chat"),
      item("sheet", "Switch sprite / animation view", s.sheet),
      item("fit", "Fit canvas"),
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
      item("reset-layers", "Reset layer view"),
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
      item("review-analyze", "Analyze current frame"),
      item("review-sheet", "Review sheet"),
      item("review-animation", "Audit animation"),
    ],
  },
  item("settings", "Settings…"),
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
        height: kind === "settings" ? 440 : 280,
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
watch(doc, (d) =>
  frame.status.update("pixloom.document", {
    text: d
      ? `${d.width} × ${d.height} · ${d.frames.length} frame${d.frames.length === 1 ? "" : "s"}`
      : "No artwork",
  }),
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
  if (event.code === "Space") {
    event.preventDefault();
    s.playing = !s.playing;
    s.mode = "composite";
    s.sheet = false;
  } else if (event.key === "ArrowRight") {
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
function tick(now: number) {
  if (last) editor.tick(now - last);
  last = now;
  animation = requestAnimationFrame(tick);
}
onMounted(async () => {
  window.addEventListener("keydown", key);
  await editor.refresh();
  await call("desktop_ready");
  timer = setInterval(editor.refresh, 300);
  animation = requestAnimationFrame(tick);
  await applyTheme(
    frame,
    native ? await call<ThemeConfig>("theme_config") : defaultTheme,
  );
  if (native) {
    unlisten = await listen("pixloom:update", editor.refresh);
    unclose = await getCurrentWindow().onCloseRequested(async (event) => {
      if (
        doc.value?.dirty &&
        !window.confirm("Discard unsaved artwork and close Pixloom?")
      )
        event.preventDefault();
    });
  }
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", key);
  clearInterval(timer);
  cancelAnimationFrame(animation);
  unlisten?.();
  unclose?.();
  frame.dispose();
});
</script>
<template>
  <FrameShell
    :frame="frame"
    :title-text="title"
    :show-search="false"
    :menu-items="menu"
    ><CanvasView
  /></FrameShell>
</template>
