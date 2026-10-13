import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtempSync,writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { callbackSites,callbackWriteAt } from './callback-sites.mjs';
test('write attribution resolves the exact captured native setter and excludes a shadowed name',()=>{
  const path=join(mkdtempSync(join(tmpdir(),'solid-callback-sites-')),'input.ts');
  writeFileSync(path,`import { createSignal as signal } from 'solid-js'; const h = {}; let pending: unknown;
const [read, write] = signal(1); const first = () => { h.samples.push({id:0}); write(2); };
const second = () => { h.samples.push({id:1}); const write = (_value: number) => {}; write(2); };`);
  const sites=callbackSites(path); assert.equal(sites.writes.length,1); assert.equal(sites.writes[0].id,0);
  const position=sites.source.getLineAndCharacterOfPosition(sites.writes[0].start);
  assert.equal(callbackWriteAt({originalLocation:{path,line:position.line+1,column:position.character+1}},path,sites).id,0);
  assert.equal(callbackWriteAt({originalLocation:{path:'/different',line:2,column:1}},path,sites),null);
});
