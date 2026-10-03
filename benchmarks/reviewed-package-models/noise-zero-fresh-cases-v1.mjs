// Fresh source forms authored after the lineage/filter/planner freeze.
import prior from './async-body-return-cases-v1.mjs';
const forms=[
  ['late-child','async function child(read:()=>number){await Promise.resolve();return {value:read()};}','return child(read);','target','1','2'],
  ['named-reaction','','return Promise.resolve().then(function afterResolve(unused?:void){return {value:read()};});','target','1','2'],
  ['three-bodies','async function child(read:()=>number){return Promise.resolve().then(()=>({value:read()}));}','return child(read);','target','1','2'],
  ['constant-reaction','','return Promise.resolve().then(()=>{read();return {value:9};});','control','9','9'],
  ['unescaped-wrapped-object','','const answer=({value:9});read();return (answer);','control','9','9'],
  ['mutated-object','','const answer={value:0};answer.value=read();return answer;','target','1','2'],
];
const cases=[];for(const mode of ['serial','concurrent'])for(const[name,child,body,role,initial,desired]of forms){const base=prior.find(row=>row.id===`async-body-return-${mode}-promise-object-deferred`),helper=`const h=(globalThis as any).__experiment;${child}export function makeTask(read:()=>number){return async()=>{await new Promise<void>(resolve=>setTimeout(resolve,1));h.values.calls++;${body}};}`;cases.push({...base,id:`noise-zero-fresh-${mode}-${name}`,artifactOrigin:'fresh-after-lineage-and-assertion-planner-freeze',stages:[{id:'initial',helper,role,initial,afterUpdate:initial,desired}]});}export default cases;
