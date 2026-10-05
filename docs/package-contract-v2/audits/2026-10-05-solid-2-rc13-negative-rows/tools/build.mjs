// Build rows.json, other-answers.json, the slice files and negative-rows.md.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { T } from './texts.mjs';
const REPO = process.cwd();
const OUT = REPO + '/rust/target/audit-rc13/rows';
const R9 = REPO + '/rust/target/audited-archives/solid-v2/2.0.0-rc.9/node_modules/';
const R13 = REPO + '/rust/target/audit-rc13/node_modules/';
const MANDIR = { 'solid-js': 'solid-js', '@solidjs/signals': 'solidjs-signals', '@solidjs/web': 'solidjs-web' };
const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');
const man13 = (pkg) => Object.fromEntries(JSON.parse(fs.readFileSync(`${REPO}/benchmarks/package-contract-v2/phase0/rc13/${MANDIR[pkg]}/files.json`, 'utf8')).map(e => [e.path, e]));
const rows9 = JSON.parse(fs.readFileSync(OUT + '/tools/rc9-rows.json', 'utf8'));
const located = JSON.parse(fs.readFileSync(OUT + '/tools/located.json', 'utf8'));
const M = JSON.parse(fs.readFileSync(OUT + '/tools/measurements.json', 'utf8'));
const dom = (d) => d.toLowerCase();
const scopeKind = (s) => s.startsWith('RowScope::HostTarget') ? 'host' : s.startsWith('RowScope::Arguments') ? 'args' : 'every';
function heading(i, r) {
  const k = scopeKind(r.scope);
  const suffix = k === 'host' ? ', `browser` host target' : k === 'args' ? ', argument scope' : '';
  return `### ${i}. \`${r.export}\` — \`${dom(r.domain)}\`${suffix} — archive \`${r.package}@2.0.0-rc.13\``;
}
function lineOf(buf, off) { let n = 1; for (let i = 0; i < off; i++) if (buf[i] === 10) n++; return n; }
const norm = (t) => t.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '').replace(/\b[A-Za-z_$][\w$]?\b/g, '#').replace(/\s+/g, '');
function fill(body) {
  return body.replace(/\{\{reads:([^}]+)\}\}/g, (_, k) => {
    const v = M.reads[k]; if (!v) throw new Error('no measurement ' + k);
    return ['prod', 'observe', 'dev'].map(b => `${b} ${v[b].rc9}→${v[b].rc13} functions, read-entry calls ${v[b].rc13Reads.length ? v[b].rc13Reads.join(', ') : 'none'}`).join('; ') + ` (cut: \`${v.cut}\`)`;
  }).replace(/\{\{host:([^}]+)\}\}/g, (_, k) => {
    const v = M.host[k]; if (!v) throw new Error('no host ' + k);
    return 'host references reached, rc.13 — ' + ['prod', 'observe', 'dev'].map(b => `${b}: ${Object.keys(v[b].rc13).length ? Object.entries(v[b].rc13).map(([h, fns]) => `\`${h}\` (${fns.join(', ')})`).join(', ') : 'none'}`).join('; ');
  });
}
const rowsOut = []; const sections = []; let sliceCount = 0;
rows9.forEach((r, idx) => {
  const i = idx + 1;
  const key = `${r.package}|${r.export}|${r.domain}`;
  const t = T[key]; if (!t) throw new Error('no text ' + key);
  const sec = heading(i, r) + (t.verdict === 'withheld' ? ' — **WITHHOLD**' : '');
  const cites = [];
  const tableRows = [];
  for (const c of r.citations) {
    const L = located.find(o => o.package === r.package && o.export === r.export && o.domain === r.domain && o.archive_path === c.archive_path);
    if (!L || L.rc13.hits !== 1 || !L.rc9.reproduced || !L.rc9.slice_ok || !L.rc9.file_ok) throw new Error('location problem ' + key + ' ' + c.archive_path);
    const f13 = fs.readFileSync(R13 + r.package + '/' + c.archive_path);
    const m = man13(r.package)[c.archive_path];
    if (m.sha256 !== sha(f13) || m.bytes !== f13.length) throw new Error('manifest mismatch ' + c.archive_path);
    const slice = f13.subarray(L.rc13.start, L.rc13.end);
    const s9 = fs.readFileSync(R9 + r.package + '/' + c.archive_path).subarray(c.start_byte, c.end_byte);
    const status = sha(slice) === c.slice_sha256 ? 'identical' : norm(slice.toString()) === norm(s9.toString()) ? 'short identifiers only' : 'code changed';
    const cite = { archive_path: c.archive_path, file_sha256: m.sha256, start_byte: L.rc13.start, end_byte: L.rc13.end, slice_sha256: sha(slice) };
    tableRows.push(`| ${c.archive_path} | ${lineOf(f13, L.rc13.start)}-${lineOf(f13, L.rc13.end - 1)} | ${cite.start_byte} | ${cite.end_byte} | \`${cite.slice_sha256}\` | ${c.start_byte}..${c.end_byte} \`${c.slice_sha256.slice(0, 8)}…\` | ${status} |`);
    if (t.verdict === 'granted') {
      cites.push(cite);
      const p = `${OUT}/audited-slices/solid-v2/rc13/${MANDIR[r.package]}/${c.archive_path}.${cite.start_byte}-${cite.end_byte}.slice`;
      fs.mkdirSync(path.dirname(p), { recursive: true }); fs.writeFileSync(p, slice); sliceCount++;
    }
  }
  rowsOut.push({ package: r.package, export: r.export, domain: dom(r.domain), scope: r.scope.replace(/\s+/g, ' '), verdict: t.verdict, reason: t.reason, section: sec, rc9_section: r.citations[0].section, rc9_audit: r.citations[0].audit, citations: cites });
  sections.push(`${sec}\n${fill(t.body)}\n\n**rc.13 definition per cited build** (${t.verdict === 'granted' ? 'these are the row\'s citations' : 'subject of the reading; not carried as citations'}; status compares the rc.9 cited slice):\n\n| \`archive_path\` | lines | \`start_byte\` | \`end_byte\` | \`slice_sha256\` | rc.9 range and digest | vs rc.9 |\n| --- | --- | --- | --- | --- | --- | --- |\n${tableRows.join('\n')}\n`);
});
fs.writeFileSync(OUT + '/rows.json', JSON.stringify(rowsOut, null, 2) + '\n');
const granted = rowsOut.filter(r => r.verdict === 'granted').length;
const signoff = rowsOut.map((r, i) => `| ${i + 1} | \`${r.package}\` \`${r.export}\` \`${r.domain}\`${scopeKind(r.scope) === 'host' ? ' (`browser`)' : scopeKind(r.scope) === 'args' ? ' (argument scope)' : ''} | **${r.verdict.toUpperCase()}** | ${r.reason} |`).join('\n');
const head = fs.readFileSync(OUT + '/tools/head.md', 'utf8').replace('{{SIGNOFF}}', signoff).replace(/\{\{GRANTED\}\}/g, String(granted)).replace(/\{\{WITHHELD\}\}/g, String(rowsOut.length - granted));
const tail = fs.readFileSync(OUT + '/tools/tail.md', 'utf8');
fs.writeFileSync(OUT + '/negative-rows.md', head + '\n' + sections.join('\n---\n\n') + '\n---\n\n' + tail);
console.log('rows', rowsOut.length, 'granted', granted, 'slices', sliceCount);
