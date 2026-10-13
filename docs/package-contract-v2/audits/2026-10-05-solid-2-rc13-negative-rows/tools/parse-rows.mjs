import fs from 'node:fs';
const src = fs.readFileSync(process.cwd() + '/rust/crates/solid-dialect/src/solid_2.rs','utf8');
const lines = src.split('\n');
const start = lines.findIndex(l => l.startsWith('const NEGATIVE_ROWS'));
const end = lines.findIndex((l,i) => i>start && l.startsWith('];'));
const body = lines.slice(start+1, end).join('\n');
// split rows by "    NegativeClaimRow {" at indentation 4
const parts = body.split(/\n    NegativeClaimRow \{/).slice(1);
const rows = [];
for (const p of parts) {
  const g = (re) => { const m = p.match(re); return m ? m[1] : null; };
  const version = g(/version: (\w+),/);
  const row = {
    package: g(/package: "([^"]+)"/), version, export: g(/export: "([^"]+)"/),
    domain: g(/domain: CallClaimDomain::(\w+)/),
    scope: (p.match(/scope: ([\s\S]*?),\n        citations/)||[])[1],
    citations: [],
  };
  const cre = /AuditedCitation::Implementation \{([\s\S]*?)\n            \}/g;
  let m;
  while ((m = cre.exec(p))) {
    const c = m[1];
    const gg = (re) => { const mm = c.match(re); return mm ? mm[1] : null; };
    row.citations.push({ audit: gg(/audit: (\w+)/), section: gg(/section: "([^"]+)"/), archive_path: gg(/archive_path: "([^"]+)"/), file_sha256: gg(/file_sha256: "([^"]+)"/), start_byte: +gg(/start_byte: (\d+)/), end_byte: +gg(/end_byte: (\d+)/), slice_sha256: gg(/slice_sha256: "([^"]+)"/) });
  }
  if (/AuditedCitation::Summary/.test(p)) row.summary = true;
  rows.push(row);
}
const rc9 = rows.filter(r => r.version === 'RC9');
fs.writeFileSync(process.cwd() + '/docs/package-contract-v2/audits/2026-10-05-solid-2-rc13-negative-rows/tools/rc9-rows.json', JSON.stringify(rc9, null, 1));
console.log(rows.length, rc9.length);
for (const r of rc9) console.log(r.package, r.export, r.domain, r.scope.replace(/\s+/g,' ').slice(0,200), r.citations.length, [...new Set(r.citations.map(c=>c.audit))].join(','), r.citations.map(c=>c.archive_path).join(' '));
