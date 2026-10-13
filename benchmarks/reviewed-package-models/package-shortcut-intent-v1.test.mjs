import assert from 'node:assert/strict';
import {test} from 'node:test';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {packageShortcuts} from './package-shortcut-v1.mjs';
test('source enrollment keeps unrelated signal result flow open',()=>{
  const path=resolve('rust/target/app-import-metric/apps/package-shortcut-local-controls/node_modules/study-shortcut-control/index.js'),models=packageShortcuts(readFileSync(path,'utf8'),path).models;
  assert.equal(models.length,1);assert.equal(models[0].valueFlow,'open');assert.equal(models[0].authority,false);assert.equal(models[0].certification,false);
});
