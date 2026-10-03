// Prepared before the final bounded profile freeze; an adapted stress population.
// This measures
// repeated task execution and history loss, not a newly found application defect.
import previous from './async-body-return-cases-v1.mjs';
const cases=[];
for(const mode of ['serial','concurrent']){
  const base=previous.find(row=>row.id===`async-body-return-${mode}-promise-object-deferred`);
  const source=base.source
    .replace('const [value,set]=createSignal(1);','const [value,set]=createSignal(1);const [pulse,setPulse]=createSignal(0);')
    .replace('h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});','')
    .replace('h.update=()=>{set(2);flush();};',`h.update=async()=>{for(let index=1;index<=512;index++){setPulse(index);flush();const until=Date.now()+3000;while(h.values.calls<=index){if(Date.now()>until)throw new Error('Task did not execute');await new Promise<void>(resolve=>setTimeout(resolve,1));}await new Promise<void>(resolve=>setTimeout(resolve,1));flush();}h.values.completedCycles=512;};`)
    .replace('const result=createMemo(()=>{const read=get;','const result=createMemo(()=>{pulse();const read=get;');
  cases.push({...base,id:`bounded-history-${mode}-512-cycles`,source,artifactOrigin:'adapted-retention-challenge-before-final-bounded-profile-freeze',
    stages:[{id:'initial',helper:base.stages[0].helper,initial:null,afterUpdate:null,desired:null,role:'replay-only'}]});
}
export default cases;
