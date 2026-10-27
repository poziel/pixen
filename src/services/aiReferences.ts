import type { ChatAttachment } from '@frame/core/modules/chat';

/** Transfer selected bytes only; native code validates and owns immutable copies. */
export async function prepareReferences(attachments: readonly ChatAttachment[], signal: AbortSignal) {
  signal.throwIfAborted();
  const files = attachments.flatMap(a => a.files ?? []);
  if (!files.length && attachments.length) throw new Error('Choose image or UTF-8 text files to attach.');
  if (files.length > 8) throw new Error('Attach at most eight reference files per message.');
  if (files.some(file => file.size > 16 * 1024 * 1024)) throw new Error('Each reference file must be at most 16 MiB.');
  if (files.reduce((sum,file) => sum+file.size,0) > 24 * 1024 * 1024) throw new Error('Reference files must total at most 24 MiB per message.');
  const result: {name:string;data:string}[] = [];
  for (const file of files) {
    const bytes = new Uint8Array(await file.arrayBuffer());
    signal.throwIfAborted();
    let binary = '';
    for (let i=0;i<bytes.length;i+=8192) binary += String.fromCharCode(...bytes.subarray(i,i+8192));
    result.push({name:file.name,data:btoa(binary)});
  }
  return result;
}
