import { rgba } from './raster';
/** Only call at an intentional hex entry or a successful paint operation. */
export function rememberColor(colors: string[], color: string) {
  const normalized = '#' + rgba(color).map(value=>value.toString(16).padStart(2,'0')).join('');
  const index = colors.indexOf(normalized);
  if (index >= 0) colors.splice(index,1);
  colors.unshift(normalized);
  colors.splice(32);
}
