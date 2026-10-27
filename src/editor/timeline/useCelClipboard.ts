import {computed,ref,shallowRef,type Ref} from 'vue';
import type {Editor} from '../editor';
import {captureCels,planCelPaste,type CopiedCels} from './celClipboard';
import {groupCells,type MatrixCell} from './matrixSelection';

export function useCelClipboard(editor:Editor,selected:Ref<MatrixCell[]>,edit:(operation:unknown,exact?:boolean)=>Promise<void>,report:(message:string,error?:boolean)=>void){
  const s=editor.state,buffer=shallowRef<CopiedCels>(),copying=ref(false);
  let pending:Promise<boolean>|undefined;
  let cutting:Promise<void>|undefined;
  const canCopy=computed(()=>!!s.snapshot?.document&&!s.snapshot.document.alternate_view&&!s.working&&!copying.value&&!!selected.value.length);
  const canPaste=computed(()=>!!(buffer.value||copying.value)&&!!s.snapshot?.document&&!s.snapshot.managed&&!s.snapshot.document.alternate_view&&!s.working);
  const canCut=computed(()=>canCopy.value&&!s.snapshot?.managed&&!selected.value.some(cell=>s.snapshot?.document?.layers.find(l=>l.id===cell.layerId)?.locked));
  function copy(){
    if(!canCopy.value)return pending??Promise.resolve(false);
    const doc=s.snapshot!.document!,id=s.snapshot!.active_document,generation=doc.generation;
    const cells=selected.value.map(cell=>({...cell}));copying.value=true;
    pending=(async()=>{
      try{
        const next=await captureCels(cells,[...doc.layers].reverse().map(l=>l.id),doc.frames.map(f=>f.id),doc.width,doc.height,cell=>editor.readCel({documentId:id,layerId:cell.layerId,frameId:cell.frameId}));
        await editor.refresh();
        if(s.snapshot?.active_document!==id||s.snapshot?.document?.generation!==generation)throw new Error('Artwork changed while copying. Copy the selection again.');
        buffer.value=next;report(`Copied ${cells.length} ${cells.length===1?'cel':'cels'}.`);return true;
      }catch(error){report(String(error),true);return false;}finally{copying.value=false;}
    })();return pending;
  }
  function cut(){
    if(!canCut.value)return cutting??Promise.resolve();
    const cells=selected.value.map(cell=>({...cell})),id=s.snapshot?.active_document,generation=s.snapshot?.document?.generation;
    cutting=(async()=>{
      if(!await copy())return;
      if(id!==s.snapshot?.active_document||generation!==s.snapshot?.document?.generation){report('Artwork changed before cutting. The source was left intact.',true);return;}
      await edit(groupCells(cells,s.snapshot!.document!.frames.map(f=>f.id)).map(group=>({type:'clear_cells',layer_id:group.layerId,frame_ids:group.frameIds})),true);
    })();return cutting;
  }
  async function paste(){
    if(cutting)await cutting;
    if(copying.value)await pending;
    if(!canPaste.value||!buffer.value)return;
    const doc=s.snapshot!.document!,frameId=doc.frames[s.frame]?.id;if(!frameId||!s.layer)return;
    try{
      const ops=planCelPaste(buffer.value,selected.value,{layerId:s.layer,frameId},[...doc.layers].reverse().map(l=>l.id),doc.frames.map(f=>f.id),doc.width,doc.height);
      if(ops.some(op=>doc.layers.find(l=>l.id===op.layer_id)?.locked))throw new Error('Unlock the destination layers before pasting.');
      await edit(ops,true);
    }catch(error){report(String(error),true);}
  }
  function key(event:KeyboardEvent){
    if(!(event.ctrlKey||event.metaKey)||event.altKey||(event.target as HTMLElement).closest('input,textarea,[contenteditable=true]'))return;
    const key=event.key.toLowerCase();if(!['c','v','x'].includes(key))return;
    event.preventDefault();event.stopPropagation();if(key==='c')void copy();else if(key==='x')void cut();else void paste();
  }
  return {copy,cut,paste,key,canCopy,canCut,canPaste,copying};
}
