// New correction probes. The failed untrack controls remain in their old set.
import cases from './callback-context-transfer-cases-v1.mjs';
export default cases.filter(row=>row.id.endsWith('-untracked-write-control')).map(row=>{
  const from='untrack(()=>dispatch(callback));', to='runWithOwner(null,()=>dispatch(callback));';
  if(!row.source.includes(from))throw new Error('Expected exactly retained untrack setup');
  return {...row,id:row.id.replace('untracked-write','detached-write'),source:row.source.replace(from,to),
    provenance:{...row.provenance,pair:row.provenance.pair.replace('untracked-write','detached-write'),family:'explicit-detached-write',
      correctionOf:row.id,correction:'untrack preserves owner in installed rc.9; detach with runWithOwner(null)'}};
});
