// Authored after the warning-policy freeze. Labels are scorer inputs only.
// Both queue modes use unchanged retained published package implementations.
import previous from './async-body-return-cases-v1.mjs';
const variants=[
  ['returned-read','return {value:read()};','target','1','2',false],
  ['returned-branch','if(read()===1)return {value:1};return {value:2};','target','1','2',false],
  ['identity-result','read();return {value:9};','target','1','2',true],
  ['discarded-read','read();h.values.discarded++;return {value:9};','control','9','9',false],
  ['equal-branches','if(read()>0)return {value:9};return {value:9};','control','9','9',false],
  ['finally-replaces','try{return {value:read()};}finally{return {value:9};}','control','9','9',false],
  ['equal-catch','try{read();return {value:9};}catch{return {value:9};}','control','9','9',false],
  ['named-allocation-open','const output={value:9};read();return output;','control','9','9',false],
  ['identity-control-open','read();return {value:9};','control','1','1',true],
];
const cases=[];
for(const mode of ['serial','concurrent'])for(const[name,body,role,initial,desired,identity]of variants){
  const base=previous.find(row=>row.id===`async-body-return-${mode}-promise-object-deferred`);
  let source=base.source.replace('h.values.calls=0;','h.values.calls=0;h.values.discarded=0;h.values.identityAllocations=0;');
  if(identity)source=source.replace('function App(){','function App(){const identities=new WeakMap<object,number>();function identityOf(data:object){let id=identities.get(data);if(id===undefined){id=++h.values.identityAllocations;identities.set(data,id);}return id;}').replace('String(result().value)','String(identityOf(result()))');
  const helper=`const h=(globalThis as any).__experiment;export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;${body}};}`;
  cases.push({...base,id:`warning-accuracy-fresh-${mode}-${name}`,source,artifactOrigin:'fresh-field-relevance-and-identity-consumer-after-policy-freeze',stages:[{id:'initial',helper,initial,afterUpdate:initial,desired,role}]});
}
export default cases;
