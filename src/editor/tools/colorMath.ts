import { hexToHsv, hsvToHex, rgba } from './raster';

export const clamp = (value: number, min = 0, max = 1) => Math.max(min, Math.min(max, value));
export function srgbToLinear(value: number) {
  return value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4;
}
export function linearToSrgb(value: number) {
  return value <= .0031308 ? value * 12.92 : 1.055 * value ** (1 / 2.4) - .055;
}
export function exposeColor(bytes: readonly number[], stops: number): number[] {
  return bytes.map((value, index) => index === 3 ? value : Math.round(clamp(linearToSrgb(srgbToLinear(value / 255) * 2 ** stops)) * 255));
}
export function hsbBytes(color: { h: number; s: number; b: number; alpha: number }) {
  return [...rgba(hsvToHex(color.h,color.s/100,color.b/100)).slice(0,3), Math.round(color.alpha*255)];
}
export function bytesHsb(bytes: readonly number[], fallbackHue: number) {
  const hex = '#' + bytes.slice(0,3).map(value=>Math.round(clamp(value,0,255)).toString(16).padStart(2,'0')).join('');
  const [h,s,b] = hexToHsv(hex);
  return { space: 'hsb' as const, h: s === 0 ? fallbackHue : h, s:s*100, b:b*100, alpha:clamp(bytes[3]/255) };
}
/** Sample linear-light edits back into display sRGB for the slider's live preview. */
export function linearGradient(bytes: readonly number[], channel: number) {
  const stops = Array.from({length:17},(_,index)=>{
    const values = [...bytes]; values[channel] = Math.round(linearToSrgb(index/16)*255);
    return `rgb(${values.slice(0,3).join(' ')}) ${index/16*100}%`;
  });
  return `linear-gradient(to right, ${stops.join(', ')})`;
}
