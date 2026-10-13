import { test } from 'vitest';
import assert from 'node:assert/strict';
import { integrationPremiseProbeDigest, validateIntegrationPremises } from './lib/integration-premises.mjs';

const id = {path:'dist/index.d.ts',start:12,end:18};
const spec = () => ({package:'fixture',version:'1.0.0',solidRuntime:[],exports:{hook:{call:{closed:['returns']}}},
  premises:[{name:'integration',exports:['hook'],valueReferences:[id],configCalls:[{declaration:id,property:'history',factories:[id]}]}]});

test('integration premises require exact identities and named export scope', () => {
  validateIntegrationPremises(spec());
  validateIntegrationPremises({exports:{}});
  for (const edit of [
    s => { s.premises[0].valueReferences[0] = {...id,path:'../index.d.ts'}; },
    s => { s.premises[0].valueReferences[0] = {...id,end:12}; },
    s => { s.premises[0].valueReferences[0] = {...id,name:'guess'}; },
    s => { s.premises[0].exports = ['absent']; },
    s => { s.premises[0].configCalls[0].factories=[]; },
    s => { s.premises.push(s.premises[0]); },
  ]) {
    const value=spec();edit(value);assert.throws(()=>validateIntegrationPremises(value));
  }
});

test('premise probe freshness binds metadata, claims, pairs, runtime and artifact', () => {
  const digest=s=>integrationPremiseProbeDigest(s,'hook',{rule:'strict-read-untracked'},'bad','good',[id]);
  const original=digest(spec());
  for (const edit of [s=>{s.premises[0].name='other';},s=>{s.exports.hook.call={};},s=>{s.version='2';}]) {
    const value=spec();edit(value);assert.notEqual(digest(value),original);
  }
  assert.notEqual(integrationPremiseProbeDigest(spec(),'hook',{},'changed','good',[id]),original);
  assert.equal(integrationPremiseProbeDigest(spec(),'unpremised',{},'bad','good',[]),undefined);
});
