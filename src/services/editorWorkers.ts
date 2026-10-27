import EditorWorker from 'monaco-editor/editor/editor.worker.js?worker';
import JsonWorker from 'monaco-editor/language/json/json.worker.js?worker';

/** Bundle local workers for Pixen's two JSON preference editors. */
export function configurePreferenceEditorWorkers() {
  const scope = globalThis as typeof globalThis & { MonacoEnvironment?: import('monaco-editor').Environment };
  if (scope.MonacoEnvironment?.getWorker || scope.MonacoEnvironment?.getWorkerUrl) return;
  scope.MonacoEnvironment = {
    ...scope.MonacoEnvironment,
    getWorker(_moduleId, label) {
      return label === 'json' ? new JsonWorker() : new EditorWorker();
    },
  };
}
