import { computed, ref, watch, type Ref } from 'vue';
import type { FrameMenuItem } from '@frame/core';
import type { Editor } from '../editor';
import type { Structure, StructureKind } from '../structure';
import { directionalCopies } from './matrixActions';
import { cellKey, groupCells, type MatrixCell } from './matrixSelection';
import {previousFrameCopies} from './frameTiming';

export function useMatrixContext(editor:Editor,selected:Ref<MatrixCell[]>,choose:(cell:MatrixCell)=>void,activate:(cell:MatrixCell)=>void,layers:Structure,frames:Structure,actions:{rename:(kind:StructureKind,id:string)=>void;link:()=>void;unlink:()=>void;clear:()=>void;edit:(operation:unknown,exact?:boolean)=>Promise<void>;copy:()=>void;cut:()=>void;paste:()=>void;canCopy:()=>boolean;canCut:()=>boolean;canPaste:()=>boolean;selectFrame:(id:string)=>void;duration:(ids:string[])=>void}) {
  const s=editor.state, point=ref<{x:number;y:number}>();
  const target=ref<{kind:'cell'|StructureKind;layerId:string;frameId:string;document:string|null|undefined;generation:number}>();
  const doc=computed(()=>s.snapshot?.document);
  const frameIds=computed(()=>doc.value?.frames.map(f=>f.id)??[]);
  function open(event:MouseEvent,kind:'cell'|StructureKind,layerId:string,frameId:string){
    if((event.target as HTMLElement).closest('input'))return;
    event.preventDefault();event.stopPropagation();if(!doc.value)return;
    const cell={layerId,frameId};
    if((kind==='cell'&&selected.value.some(value=>cellKey(value)===cellKey(cell)))||(kind==='frame'&&selected.value.some(value=>value.frameId===frameId)))activate(cell);else choose(cell);
    target.value={kind,...cell,document:s.snapshot?.active_document,generation:doc.value.generation};point.value={x:event.clientX,y:event.clientY};
  }
  function sources(){const t=target.value;if(!t)return [];return t.kind==='frame'?doc.value!.layers.map(l=>({layerId:l.id,frameId:t.frameId})):selected.value;}
  const immutable=computed(()=>layers.disabled.value||target.value?.document!==s.snapshot?.active_document||target.value?.generation!==doc.value?.generation);
  const locked=computed(()=>sources().some(cell=>doc.value?.layers.find(l=>l.id===cell.layerId)?.locked));
  const items=computed<FrameMenuItem[]>(()=>{
    const t=target.value;if(!t||!doc.value)return [];
    const disabled=immutable.value, cellsDisabled=disabled||locked.value;
    const result:FrameMenuItem[]=[];
    if(t.kind==='layer')result.push({id:'rename-layer',label:'Rename Layer',disabled},{id:'duplicate-layer',label:'Duplicate Layer',disabled});
    if(t.kind==='frame')result.push({id:'rename-frame',label:'Rename Frame',disabled},{id:'duplicate-frame',label:'Duplicate Frame',disabled});
    if(t.kind==='frame')result.push({id:'select-frame',label:'Select frame cels',disabled:editor.following()});
    if(t.kind==='frame'){
      result.push({id:'duration',label:'Frame duration…',disabled});
      if(new Set(selected.value.map(cell=>cell.frameId)).size>1)result.push({id:'duration-selected',label:'Duration of selected frames…',disabled});
      result.push({id:'copy-previous',label:'Copy previous frame here',disabled:disabled||frameIds.value.indexOf(t.frameId)<1||doc.value.layers.some(layer=>layer.locked)});
    }
    if(t.kind==='cell'){
      result.push({id:'copy',label:'Copy selected cels',shortcut:'Ctrl+C',disabled:!actions.canCopy()},{id:'cut',label:'Cut selected cels',shortcut:'Ctrl+X',disabled:!actions.canCut()},{id:'paste',label:'Paste into selected cels',shortcut:'Ctrl+V',disabled:disabled||!actions.canPaste()});
      const cell=doc.value.layers.find(l=>l.id===t.layerId)?.cells?.[frameIds.value.indexOf(t.frameId)];
      result.push({id:'link',label:'Link selected cels',disabled:cellsDisabled||!groupCells(selected.value,frameIds.value).some(g=>g.frameIds.length>1)},
        {id:'unlink-this',label:'Unlink this cel',disabled:disabled||!!doc.value.layers.find(l=>l.id===t.layerId)?.locked||!cell?.link},
        {id:'unlink-selected',label:'Unlink selected cels',disabled:cellsDisabled||!selected.value.some(c=>doc.value!.layers.find(l=>l.id===c.layerId)?.cells?.[frameIds.value.indexOf(c.frameId)]?.link)});
    }
    if(t.kind!=='layer')result.push({id:'copy-right',label:'Copy to the Right',disabled:cellsDisabled||!directionalCopies(sources(),frameIds.value,1).length},{id:'copy-left',label:'Copy to the Left',disabled:cellsDisabled||!directionalCopies(sources(),frameIds.value,-1).length});
    if(t.kind==='cell')result.push({id:'clear',label:'Clear selected cels',disabled:cellsDisabled||!selected.value.length});
    result.push({id:'structure-separator',type:'separator'});
    if(t.kind!=='layer')result.push({id:'delete-frame',label:'Delete Frame',disabled:disabled||doc.value.frames.length<2});
    if(t.kind!=='frame')result.push({id:'delete-layer',label:'Delete Layer',disabled:disabled||doc.value.layers.length<2});
    return result;
  });
  function action(item:FrameMenuItem){
    const t=target.value,definition=items.value.find(value=>value.id===item.id);point.value=undefined;
    if(!t||!definition||definition.type==='separator'||definition.disabled||immutable.value)return;
    if(item.id==='rename-layer')actions.rename('layer',t.layerId);
    if(item.id==='rename-frame')actions.rename('frame',t.frameId);
    if(item.id==='duplicate-layer')void layers.duplicate(t.layerId);
    if(item.id==='duplicate-frame')void frames.duplicate(t.frameId);
    if(item.id==='delete-layer')layers.requestDelete(t.layerId);
    if(item.id==='delete-frame')frames.requestDelete(t.frameId);
    if(item.id==='link')actions.link();
    if(item.id==='copy')actions.copy();
    if(item.id==='cut')actions.cut();
    if(item.id==='select-frame')actions.selectFrame(t.frameId);
    if(item.id==='duration')actions.duration([t.frameId]);
    if(item.id==='duration-selected')actions.duration([...new Set(selected.value.map(cell=>cell.frameId))]);
    if(item.id==='copy-previous')void actions.edit(previousFrameCopies(doc.value!.layers.map(layer=>layer.id),frameIds.value,t.frameId),true);
    if(item.id==='paste')actions.paste();
    if(item.id==='unlink-selected')actions.unlink();
    if(item.id==='unlink-this')void actions.edit({type:'unlink_cells',layer_id:t.layerId,frame_ids:[t.frameId]},true);
    if(item.id==='copy-right'||item.id==='copy-left')void actions.edit(directionalCopies(sources(),frameIds.value,item.id==='copy-right'?1:-1),true);
    if(item.id==='clear')actions.clear();
  }
  watch(()=>s.snapshot?.active_document,()=>{point.value=undefined;target.value=undefined;});
  return {point,items,open,action,close:()=>{point.value=undefined;}};
}
