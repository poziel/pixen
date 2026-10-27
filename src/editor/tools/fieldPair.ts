export type FieldPair = [number, number];
export function updatePair(current: FieldPair, index: 0|1, value: number, ratio: FieldPair|null, min: number): FieldPair {
  if(!Number.isFinite(value))return [...current];
  const result:FieldPair=[...current];
  result[index]=Math.max(min,Math.round(value));
  if(ratio&&ratio[0]>0&&ratio[1]>0)result[index===0?1:0]=Math.max(min,Math.round(result[index]*ratio[index===0?1:0]/ratio[index]));
  else if(ratio)result[index===0?1:0]=Math.max(min,current[index===0?1:0]+result[index]-current[index]);
  return result;
}
