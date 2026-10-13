// Re-hash every citation's slice file and rc.13 range; compare file digest with files.json.
import fs from 'node:fs';
import crypto from 'node:crypto';
const REPO = process.cwd();
const OUT = REPO + '/rust/target/audit-rc13/rows';
const R13 = REPO + '/rust/target/audit-rc13/node_modules/';
const MANDIR = { 'solid-js': 'solid-js', '@solidjs/signals': 'solidjs-signals', '@solidjs/web': 'solidjs-web' };
const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');
const rows = JSON.parse(fs.readFileSync(OUT + '/rows.json', 'utf8'));
const md = fs.readFileSync(OUT + '/negative-rows.md', 'utf8');
let n = 0, bad = 0;
for (const r of rows) {
  if (!md.split('\n').includes(r.section)) { console.log('MISSING SECTION', r.section); bad++; }
  if (r.verdict === 'granted' && r.citations.length === 0) { console.log('NO CITATIONS', r.section); bad++; }
  if (r.verdict === 'withheld' && r.citations.length !== 0) { console.log('WITHHELD WITH CITATIONS', r.section); bad++; }
  const man = Object.fromEntries(JSON.parse(fs.readFileSync(`${REPO}/benchmarks/package-contract-v2/phase0/rc13/${MANDIR[r.package]}/files.json`, 'utf8')).map(e => [e.path, e]));
  for (const c of r.citations) {
    n++;
    const file = fs.readFileSync(R13 + r.package + '/' + c.archive_path);
    const range = file.subarray(c.start_byte, c.end_byte);
    const slice = fs.readFileSync(`${OUT}/audited-slices/solid-v2/rc13/${MANDIR[r.package]}/${c.archive_path}.${c.start_byte}-${c.end_byte}.slice`);
    const ok = sha(file) === c.file_sha256 && man[c.archive_path]?.sha256 === c.file_sha256 && man[c.archive_path]?.bytes === file.length && sha(range) === c.slice_sha256 && sha(slice) === c.slice_sha256 && Buffer.compare(range, slice) === 0;
    if (!ok) { bad++; console.log('BAD', r.export, r.domain, c.archive_path); }
  }
}
console.log(`${rows.length} rows (${rows.filter(r => r.verdict === 'granted').length} granted, ${rows.filter(r => r.verdict === 'withheld').length} withheld), ${n} citations checked, ${bad} problems`);
process.exit(bad ? 1 : 0);
