import { srgbToLinear } from './colorMath';

function luminance(color: readonly number[]) {
  return color.slice(0,3).reduce((sum,value,index)=>sum+srgbToLinear(value/255)*[.2126,.7152,.0722][index],0);
}
/** Choose black or white for maximum contrast against the sampled center pixel. */
export function samplerBorder(sample: readonly number[]): readonly number[] {
  const b=luminance(sample);
  return (b+.05)/.05 >= 1.05/(b+.05) ? [0,0,0] : [255,255,255];
}
