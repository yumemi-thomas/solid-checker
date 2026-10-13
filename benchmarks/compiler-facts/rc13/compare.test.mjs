import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {compareRows} from './compare.mjs';

const hash=value=>createHash('sha256').update(value).digest('hex');
function sample(){
  const request={id:'control|dom',source:'const x = <div />;',options:{generate:'dom',hoistProps:true,components:false,bindings:false}};
  const plain={status:'compiled',code:'const x = template();',map:'{"version":3}',css:null,cssHash:null};
  const trace={version:3,identity:{compiler:{upstream_revision:'5efaf260becb32293f2bcb4d32f8be72be6de674',implementation_revision:'c04c48779812d3d87166da3741c625748458c62f'},
    source_sha256:hash(request.source),output_sha256:hash(plain.code),source_map_sha256:hash(plain.map),
    config:{hoist_props:true,source_names_components:false,source_names_bindings:false}},sites:[],generated_operations:[]};
  return [[request],[{id:request.id,plain}],[{id:request.id,plain:structuredClone(plain),traced:structuredClone(plain),trace}]];
}
test('complete comparison accepts object key order differences',()=>{
  const args=sample();args[2][0].plain=Object.fromEntries(Object.entries(args[2][0].plain).reverse());
  assert.equal(compareRows(...args).length,1);
});
for(const [name,change] of [
  ['compiler output drift',args=>{args[2][0].plain.code+=' changed';}],
  ['trace output drift',args=>{args[2][0].traced.map='different map';}],
  ['missing trace',args=>{args[2][0].trace=null;}],
  ['stale source',args=>{args[0][0].source+=' changed';}],
  ['stale output digest',args=>{args[2][0].trace.identity.output_sha256=hash('wrong');}],
  ['stale map digest',args=>{args[2][0].trace.identity.source_map_sha256=null;}],
  ['different effective option',args=>{args[0][0].options.hoistProps=false;}],
  ['old compiler base',args=>{args[2][0].trace.identity.compiler.upstream_revision='old';}],
  ['old fact implementation',args=>{args[2][0].trace.identity.compiler.implementation_revision='old';}],
  ['old trace protocol',args=>{args[2][0].trace.version=2;}],
  ['incomplete output',args=>{args[1].pop();}],
  ['mismatched identity',args=>{args[2][0].id='other';}],
  ['duplicate requests',args=>{for(const array of args)array.push(structuredClone(array[0]));}],
  ['unsupported facts admitted',args=>{args[0][0].options.generate='universal';}],
])test(`rejects ${name}`,()=>{const args=sample();change(args);assert.throws(()=>compareRows(...args));});
test('unsupported fact mode refuses while preserving ordinary output',()=>{
  const args=sample();args[0][0].options.generate='dynamic';args[2][0].traced={status:'refused',error:'unsupported facts'};args[2][0].trace=null;
  assert.equal(compareRows(...args)[0].supported,false);
});
test('unsupported fact mode may omit the absent trace field',()=>{
  const args=sample();args[0][0].options.generate='universal';args[2][0].traced={status:'refused'};delete args[2][0].trace;
  assert.equal(compareRows(...args)[0].supported,false);
});
