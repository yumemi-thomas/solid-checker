// Fresh consumers after the visible-replay profile freeze. A visible execution
// counter is an intentional control, not an expected reactive result defect.
import previous from './async-body-return-cases-v1.mjs';
const cases=[];
for(const mode of ['serial','concurrent'])for(const kind of ['renamed-bindings','visible-execution-counter','object-getter','optional-getter']){
  const base=previous.find(row=>row.id===`async-body-return-${mode}-promise-object-deferred`);let source=base.source,helper=base.stages[0].helper;
  if(kind==='renamed-bindings'){
    source=source.replaceAll('makeTask','buildWork').replace('const read=get;','const suppliedReader=get;').replace('buildWork(read)','buildWork(suppliedReader)');
    helper=helper.replaceAll('makeTask','buildWork').replaceAll('read:','supplied:').replaceAll('read()','supplied()');
  }
  if(kind==='visible-execution-counter'){
    source=source.replace('h.values.calls=0;','const [auditCount,setAuditCount]=createSignal(0);h.tick=()=>{setAuditCount(v=>v+1);flush();};h.values.calls=0;').replace("<p id='value'>{String(result().value)}</p>","<div><p id='value'>{String(result().value)}</p><small id='audit'>Executed {auditCount()}</small></div>");
    helper=helper.replace('return Promise.resolve({value:read()});','read();h.tick();return {value:9};');
  }
  if(kind==='object-getter'){
    source=source.replace('return value();','return {value:value()};');helper=helper.replaceAll('read:()=>number','read:()=>{value:number}').replace('return Promise.resolve({value:read()});','return Promise.resolve({value:read().value});');
  }
  if(kind==='optional-getter')source=source.replace('const get=()=>','const get=(unused=0)=>');
  const control=kind==='visible-execution-counter',initial=control?'9':'1';
  cases.push({...base,id:`capture-replay-challenge-${mode}-${kind}`,source,artifactOrigin:'fresh-source-binding-and-replay-confound-consumer',stages:[{id:'initial',helper,initial,afterUpdate:initial,desired:control?'9':'2',role:control?'control':'target'}]});
}
export default cases;
