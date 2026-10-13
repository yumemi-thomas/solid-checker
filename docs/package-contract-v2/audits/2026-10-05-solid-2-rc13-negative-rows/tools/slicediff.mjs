// Diff the rc.9 cited slice against the rc.13 located slice, for (export, archive_path).
import fs from 'node:fs';
import { execFileSync } from 'node:child_process';
const REPO = process.cwd();
const R9 = REPO + '/rust/target/audited-archives/solid-v2/2.0.0-rc.9/node_modules/';
const R13 = REPO + '/rust/target/audit-rc13/node_modules/';
const T = REPO + '/rust/target/audit-rc13/rows/tools/tmp';
const loc = JSON.parse(fs.readFileSync(REPO + '/rust/target/audit-rc13/rows/tools/located.json', 'utf8'));
const [,, exp, ap] = process.argv;
const r = loc.find(o => o.export === exp && (!ap || o.archive_path === ap));
for (const o of loc.filter(o => o.export === exp && (!ap || o.archive_path === ap) && o.domain === r.domain)) {
  const a = fs.readFileSync(R9 + o.package + '/' + o.archive_path).subarray(o.rc9.start, o.rc9.end).toString('utf8');
  const b = fs.readFileSync(R13 + o.package + '/' + o.archive_path).subarray(o.rc13.start, o.rc13.end).toString('utf8');
  console.log(`===== ${o.package} ${exp} ${o.archive_path} rc9 ${o.rc9.start}-${o.rc9.end} rc13 ${o.rc13.start}-${o.rc13.end} ${o.rc13.identical ? 'IDENTICAL' : ''}`);
  if (o.rc13.identical) continue;
  fs.writeFileSync(T + '/sa.js', a.replace(/([;{}])\s*/g, '$1\n')); fs.writeFileSync(T + '/sb.js', b.replace(/([;{}])\s*/g, '$1\n'));
  try { execFileSync('diff', ['-u', T + '/sa.js', T + '/sb.js'], { stdio: 'pipe' }); console.log('(identical after reflow)'); } catch (e) { console.log(e.stdout.toString().split('\n').slice(2).join('\n')); }
}
