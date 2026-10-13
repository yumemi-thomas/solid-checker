// usage: node show.mjs <build> <name...>  (env V=rc9|rc13|diff, default diff)
import { load } from './closure.mjs';
import fs from 'node:fs';
import { execFileSync } from 'node:child_process';
const [,, build, ...names] = process.argv;
const T = process.cwd() + '/docs/package-contract-v2/audits/2026-10-05-solid-2-rc13-negative-rows/tools/tmp';
fs.mkdirSync(T, { recursive: true });
const a = load('rc9', build), b = load('rc13', build);
const V = process.env.V || 'diff';
for (const n of names) {
  const x = (a.get(n) || []), y = (b.get(n) || []);
  console.log(`===== ${n} rc9: ${x.map(e => e.file).join(',') || '-'} (${x.length})  rc13: ${y.map(e => e.file).join(',') || '-'} (${y.length})`);
  const tx = x.map(e => e.text).join('\n//----\n') + '\n', ty = y.map(e => e.text).join('\n//----\n') + '\n';
  if (V === 'rc9') { console.log(tx); continue; }
  if (V === 'rc13') { console.log(ty); continue; }
  fs.writeFileSync(T + '/a.js', tx); fs.writeFileSync(T + '/b.js', ty);
  try { execFileSync('diff', ['-u', T + '/a.js', T + '/b.js'], { stdio: 'pipe' }); console.log('(identical)'); }
  catch (e) { console.log(e.stdout.toString().split('\n').slice(2).join('\n')); }
}
