export type ExportKind = 'frame' | 'sheet' | 'animation' | 'video' | 'project';
export const exportKinds = [
  { value: 'frame', label: 'Current frame' },
  { value: 'sheet', label: 'Sprite sheet' },
  { value: 'animation', label: 'Animation' },
  { value: 'video', label: 'Video' },
  { value: 'project', label: 'Editable project' },
];
export const exportFormats = [
  { value: 'png', label: 'PNG' }, { value: 'jpeg', label: 'JPEG' },
  { value: 'gif', label: 'GIF' }, { value: 'webp', label: 'WebP' },
  { value: 'svg', label: 'SVG' }, { value: 'bmp', label: 'BMP' },
  { value: 'tiff', label: 'TIFF' }, { value: 'apng', label: 'APNG' },
  { value: 'mp4', label: 'MP4' }, { value: 'webm', label: 'WebM' },
  { value: 'pixen.json', label: 'Pixen JSON (.pixen.json)' },
];
const supported: Record<ExportKind, readonly string[]> = {
  frame: ['png', 'jpeg', 'gif', 'webp', 'svg', 'bmp', 'tiff', 'apng'],
  sheet: ['png', 'jpeg', 'gif', 'webp', 'svg', 'bmp', 'tiff', 'apng'],
  animation: ['gif', 'webp', 'apng'], video: ['mp4', 'webm'], project: ['pixen.json'],
};
export interface ExportEntry {
  id: number;
  mode: ExportKind;
  format: string;
  frame: number;
  scale: number;
  gridAxis: 'columns' | 'rows';
  gridCount: number;
  opaque: boolean;
  background: string;
  loopAnimation: boolean;
}
export function createExportEntry(id: number, count: number, selectedFrame: number): ExportEntry {
  return { id, mode: 'frame', format: 'png', frame: Math.min(selectedFrame + 1, count), scale: 1,
    gridAxis: 'columns', gridCount: Math.ceil(Math.sqrt(count)), opaque: false,
    background: '#ffffff', loopAnimation: true };
}
export function supportsFormat(kind: ExportKind, format: string) { return supported[kind].includes(format); }
export function formatOptions(kind: ExportKind) {
  return exportFormats.map(option => ({ ...option, disabled: !supportsFormat(kind, option.value) }));
}
export function changeExportKind(entry: ExportEntry, kind: ExportKind) {
  entry.mode = kind;
  if (!supportsFormat(kind, entry.format)) entry.format = supported[kind][0]!;
}
export function needsBackground(format: string) { return ['jpeg', 'bmp', 'mp4', 'webm'].includes(format); }
export function exportDimensions(entry: ExportEntry, count: number, width: number, height: number) {
  const columns = entry.mode === 'sheet' ? (entry.gridAxis === 'columns' ? entry.gridCount : Math.ceil(count / entry.gridCount)) : 1;
  const rows = entry.mode === 'sheet' ? (entry.gridAxis === 'rows' ? entry.gridCount : Math.ceil(count / entry.gridCount)) : 1;
  return { columns, rows, width: width * entry.scale * columns, height: height * entry.scale * rows };
}
export function validExport(entry: ExportEntry, count: number, width: number, height: number) {
  if (!supportsFormat(entry.mode, entry.format)) return false;
  if (entry.mode === 'project') return true;
  const size = exportDimensions(entry, count, width, height);
  return Number.isInteger(entry.scale) && entry.scale >= 1 && entry.scale <= 64
    && size.width <= 4096 && size.height <= 4096 && size.width * size.height <= 4194304
    && (entry.mode !== 'frame' || (Number.isInteger(entry.frame) && entry.frame >= 1 && entry.frame <= count))
    && (entry.mode !== 'sheet' || (Number.isInteger(entry.gridCount) && entry.gridCount >= 1 && entry.gridCount <= count))
    && (entry.mode !== 'video' || count > 1)
    && (!(entry.opaque || needsBackground(entry.format)) || /^#[a-f\d]{6}$/i.test(entry.background));
}
export function validExportName(name: string) {
  return name.length > 0 && name.length <= 120 && name === name.trim() && !/[<>:"/\\|?*\x00-\x1f]/.test(name)
    && !/[. ]$/.test(name) && !/^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(name);
}
export function exportNames(name: string, entries: readonly ExportEntry[]) {
  const totals = new Map<string, number>();
  for (const entry of entries) totals.set(entry.format, (totals.get(entry.format) ?? 0) + 1);
  const used = new Set<string>();
  return entries.map(entry => {
    const stem = `${name}${totals.get(entry.format)! > 1 ? `-${entry.mode}` : ''}`;
    let output = `${stem}.${entry.format}`, suffix = 2;
    while (used.has(output.toLowerCase())) output = `${stem}-${suffix++}.${entry.format}`;
    used.add(output.toLowerCase());
    return output;
  });
}
