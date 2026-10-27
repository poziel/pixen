import { onBeforeUnmount, ref, watch, type Ref } from 'vue';
import type { Structure, StructureKind } from '../structure';
import { moveHeader } from './matrixActions';

/** Matrix headers have two axes; cell pointer gestures remain owned by selection. */
export function useMatrixReorder(root: Ref<HTMLElement|undefined>, layer: Structure, frame: Structure) {
  const preview=ref<{kind:StructureKind;ids:string[];x:number;y:number}>();
  const indicator=ref<{kind:StructureKind;id:string;after:boolean}>(), dragging=ref(false);
  let pending:{kind:StructureKind;id:string;pointer:number;x:number;y:number;element:HTMLElement}|undefined;
  let listeners:AbortController|undefined, animation=0, suppress=false;
  let point={x:0,y:0};
  const controller=(kind:StructureKind)=>kind==='layer'?layer:frame;
  function locate() {
    if(!pending||!root.value)return;
    const viewport=root.value.closest<HTMLElement>('.frame-scroll-area-viewport'), box=viewport?.getBoundingClientRect();
    if(!box||point.x<box.left||point.x>box.right||point.y<box.top||point.y>box.bottom){indicator.value=undefined;return;}
    const headers=[...root.value.querySelectorAll<HTMLElement>(`[data-structure-kind="${pending.kind}"]`)];
    const hit=headers.find(header=>{const rect=header.getBoundingClientRect();return pending!.kind==='layer'?point.y>=rect.top&&point.y<=rect.bottom:point.x>=rect.left&&point.x<=rect.right;});
    if(!hit){indicator.value=undefined;return;}
    const rect=hit.getBoundingClientRect();
    indicator.value={kind:pending.kind,id:hit.dataset.structureId!,after:pending.kind==='layer'?point.y>rect.top+rect.height/2:point.x>rect.left+rect.width/2};
  }
  function scroll() {
    if(!pending||!dragging.value)return;
    const viewport=root.value?.closest<HTMLElement>('.frame-scroll-area-viewport'),box=viewport?.getBoundingClientRect();
    if(viewport&&box){const axis=pending.kind==='layer'?'y':'x';const position=axis==='x'?point.x:point.y;const start=axis==='x'?box.left:box.top,end=axis==='x'?box.right:box.bottom;const delta=position<start+24?-10:position>end-24?10:0;if(delta){viewport.scrollBy(axis==='x'?delta:0,axis==='y'?delta:0);locate();}}
    animation=requestAnimationFrame(scroll);
  }
  function down(event:PointerEvent,kind:StructureKind,id:string) {
    if(event.button!==0||controller(kind).disabled.value||(event.target as HTMLElement).closest('input,[data-reorder-ignore]'))return;
    stop();pending={kind,id,pointer:event.pointerId,x:event.clientX,y:event.clientY,element:event.currentTarget as HTMLElement};
    point={x:event.clientX,y:event.clientY};listeners=new AbortController();const options={signal:listeners.signal};
    window.addEventListener('pointermove',move,options);window.addEventListener('pointerup',up,options);
    window.addEventListener('pointercancel',()=>stop(),options);window.addEventListener('blur',()=>stop(),options);
    window.addEventListener('keydown',event=>{if(event.key==='Escape'){event.preventDefault();stop();}},options);
  }
  function move(event:PointerEvent) {
    if(!pending||event.pointerId!==pending.pointer)return;
    point={x:event.clientX,y:event.clientY};
    if(!dragging.value){if(Math.hypot(point.x-pending.x,point.y-pending.y)<5)return;dragging.value=true;pending.element.setPointerCapture(event.pointerId);animation=requestAnimationFrame(scroll);}
    preview.value={kind:pending.kind,ids:[pending.id],x:point.x,y:point.y};
    event.preventDefault();locate();
  }
  function up(event:PointerEvent) {
    if(!pending||event.pointerId!==pending.pointer)return;
    if(dragging.value){
      event.preventDefault();suppress=true;setTimeout(()=>{suppress=false;},0);
      if(indicator.value){const c=controller(pending.kind),order=c.display.value.map(item=>item.id);const next=moveHeader(order,pending.id,indicator.value.id,indicator.value.after);if(next.some((id,index)=>id!==order[index]))c.reorder(next);}
    }
    stop();
  }
  function stop(){listeners?.abort();listeners=undefined;cancelAnimationFrame(animation);const previous=pending;pending=undefined;if(previous?.element.hasPointerCapture(previous.pointer))previous.element.releasePointerCapture(previous.pointer);dragging.value=false;indicator.value=undefined;preview.value=undefined;}
  function click(event:MouseEvent){if(suppress){event.preventDefault();event.stopImmediatePropagation();}}
  function key(event:KeyboardEvent,kind:StructureKind,id:string){
    const keys=kind==='layer'?['ArrowUp','ArrowDown']:['ArrowLeft','ArrowRight'];
    if(!event.altKey||!keys.includes(event.key)||controller(kind).disabled.value||(event.target as HTMLElement).closest('input'))return;
    event.preventDefault();event.stopPropagation();const c=controller(kind),order=c.display.value.map(item=>item.id),index=order.indexOf(id),target=order[index+(event.key===keys[0]?-1:1)];
    if(target)c.reorder(moveHeader(order,id,target,event.key===keys[1]));
  }
  watch(()=>[layer.disabled.value,frame.disabled.value],()=>{if(pending&&controller(pending.kind).disabled.value)stop();});
  onBeforeUnmount(stop);
  return {preview,indicator,dragging,down,click,key,stop};
}
