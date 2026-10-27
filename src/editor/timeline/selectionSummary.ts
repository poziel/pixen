export function selectionSummary(cells: readonly {layerId:string;frameId:string}[], layers: readonly {id:string;name:string}[]): string {
  const unique=new Map(cells.map(cell=>[JSON.stringify([cell.layerId,cell.frameId]),cell]));
  if(!unique.size)return 'No cels selected';
  const ids=new Set([...unique.values()].map(cell=>cell.layerId));
  const label=ids.size===1?layers.find(layer=>ids.has(layer.id))?.name??'Layer':`${ids.size} layers`;
  return `${label} · ${unique.size} ${unique.size===1?'cel':'cels'}`;
}
