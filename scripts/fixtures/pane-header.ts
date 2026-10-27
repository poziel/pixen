import { createApp, defineComponent, h, nextTick } from 'vue';
import { createFrame, FrameShell } from '@frame/core';

/** Exercise the public header opt-out beside a second, independently mounted Frame host. */
export async function verifyPaneHeader() {
  const frame = createFrame({ panelViewBar: { visible: false } }), other = createFrame();
  const content = defineComponent(() => () => h('p', 'Custom pane content'));
  frame.registerBottomTab({ id: 'custom', label: 'Custom', component: content, showHeader: false });
  frame.registerBottomTab({ id: 'standard', label: 'Standard', component: content });
  other.registerBottomTab({ id: 'custom', label: 'Custom', component: content });
  const host = document.createElement('div');
  Object.assign(host.style, { position: 'fixed', left: '-10000px', width: '640px', height: '480px' });
  document.body.append(host);
  const app = createApp({ render: () => h(FrameShell, { frame }) });
  try {
    frame.openBottomPanel('custom'); app.mount(host); await nextTick();
    const hidden = host.querySelectorAll('#bottom-panel .panel-header').length;
    frame.openBottomPanel('standard'); await nextTick();
    const standard = host.querySelectorAll('#bottom-panel .panel-header').length;
    frame.openBottomPanel('custom'); await nextTick();
    await frame.commands.execute('frame.panel.maximize');
    const maximized = frame.layout.maximizedRegion === 'bottom';
    frame.closeBottomPanel(); frame.openBottomPanel('custom'); await nextTick();
    const reopened = host.querySelectorAll('#bottom-panel .panel-header').length;
    frame.moveView('custom', 'secondary'); frame.openView('custom'); await nextTick();
    const moved = host.querySelectorAll('#secondary-panel .panel-header').length;
    const isolated = other.bottomTabs.value[0]?.showHeader === undefined;
    return { hidden, standard, maximized, reopened, moved, isolated };
  } finally { app.unmount(); host.remove(); frame.dispose(); other.dispose(); }
}
