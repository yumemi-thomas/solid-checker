// usage: node defdiff.mjs <pkg> <rel> name...  (diff rc9 vs rc13 top-level definition text)
import fs from 'node:fs';
import { execFileSync } from 'node:child_process';
import { topDefs } from './topwalk.mjs';
const [,, pkg, rel, ...names] = process.argv;
const T = process.cwd() + '/docs/package-contract-v2/audits/2026-10-05-solid-2-rc13-negative-rows/tools/tmp';
const A = topDefs('rc9', pkg, rel).defs, B = topDefs('rc13', pkg, rel).defs;
for (const n of names) {
  const a = A.get(n)?.text ?? '', b = B.get(n)?.text ?? '';
  console.log(`===== ${pkg}/${rel} ${n}`);
  if (process.env.V === 'rc13') { console.log(b); continue; }
  fs.writeFileSync(T + '/da.js', a + '\n'); fs.writeFileSync(T + '/db.js', b + '\n');
  try { execFileSync('diff', ['-u', T + '/da.js', T + '/db.js'], { stdio: 'pipe' }); console.log('(identical)'); } catch (e) { console.log(e.stdout.toString().split('\n').slice(2).join('\n')); }
}
