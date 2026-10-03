// Authored after the body-return detector freeze. These are consumer challenges,
// using unchanged retained published queue/runtime bytes and real declarations.
import previous from './async-callback-slot-cases-v1.mjs';
const cases=[];
const bodies={
  'promise-object':'return Promise.resolve({value:read()});',
  'thenable-read-before-return':'const value=read();return {then(resolve:(value:{value:number})=>void){resolve({value});}};',
  'function-object':'return Object.assign(()=>0,{value:read()});',
  'awaited-child-object':'return await child(read);',
  'adopted-child-object':'return child(read);',
  'reaction-read':'return Promise.resolve().then(()=>({value:read()}));',
  'constant-object':'read();return {value:9};',
  'caught-rejected-promise':'read();return Promise.reject(new Error("expected adoption rejection"));',
  'caught-throwing-then-getter':'read();return Object.defineProperty({value:0},"then",{get(){h.values.thenGets++;throw new Error("expected getter rejection");}});',
  'never-settling-promise':'read();return new Promise<{value:number}>(()=>{});',
};
for(const mode of ['serial','concurrent'])for(const [name,body]of Object.entries(bodies))for(const bad of name.includes('constant')||name.startsWith('caught')||name==='never-settling-promise'?[true]:[true,false]){
  const original=previous.find(row=>row.id===`async-callback-slot-${mode}-object-${bad?'target':'capture'}`);
  let source=original.source.replace('h.values.calls=0;','h.values.calls=0;h.values.thenGets=0;');
  if(name.startsWith('caught'))source=source.replace('return await queue.enqueue(makeTask(read));','try{return await queue.enqueue(makeTask(read));}catch{return {value:9};}');
  if(name==='never-settling-promise')source=source.replace('<p>waiting</p>','<p id="value">waiting</p>');
  const helper=`const h=(globalThis as any).__experiment;async function child(read:()=>number){await Promise.resolve();return {value:read()};}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;${body}};}`;
  const constant=name==='constant-object'||name.startsWith('caught'),pending=name==='never-settling-promise',role=bad&&!constant&&!pending?'target':'control',initial=pending?'waiting':constant?'9':'1',afterUpdate=bad||constant||pending?initial:'2';
  cases.push({...original,id:`async-body-return-${mode}-${name}-${bad?'deferred':'capture'}`,source,artifactOrigin:'fresh-body-return-consumer-after-detector-freeze',stages:[{id:'initial',helper,initial,afterUpdate,desired:role==='target'?'2':afterUpdate,role}]});
}
export default cases;
