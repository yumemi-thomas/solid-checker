// For each rc.9 citation: verify rc.9 bytes, infer the slice convention and defined
// name, then locate the same definition in the rc.13 file and compute its range.
import fs from 'node:fs';
import crypto from 'node:crypto';
const REPO = process.cwd();
const R9 = REPO + '/rust/target/audited-archives/solid-v2/2.0.0-rc.9/node_modules/';
const R13 = REPO + '/rust/target/audit-rc13/node_modules/';
const PKGDIR = { 'solid-js': 'solid-js', '@solidjs/signals': '@solidjs/signals', '@solidjs/web': '@solidjs/web' };
const MAN = { 'solid-js': 'solid-js', '@solidjs/signals': 'solidjs-signals', '@solidjs/web': 'solidjs-web' };
const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');
const man = (rel, pkg) => Object.fromEntries(JSON.parse(fs.readFileSync(`${REPO}/benchmarks/package-contract-v2/phase0/${rel}/${MAN[pkg]}/files.json`, 'utf8')).map(e => [e.path, e]));
const rows = JSON.parse(fs.readFileSync(REPO + '/rust/target/audit-rc13/rows/tools/rc9-rows.json', 'utf8'));
const out = [];
function findDef(buf, name, conv) {
  const text = buf.toString('latin1');
  const pats = [new RegExp(`(^|[\\n;}\\s/])function ${name.replace('$','\\$')}\\(`, 'g'), new RegExp(`(^|[\\n;}\\s])const ${name.replace('$','\\$')} = `, 'g')];
  const hits = [];
  for (const p of pats) { let m; while ((m = p.exec(text))) hits.push(m.index + m[1].length); }
  return hits;
}
// end of a definition starting at idx: brace matching via TypeScript parser
import { createRequire } from 'node:module';
const require = createRequire(REPO + '/packages/cli/');
const ts = require('typescript');
function stmtAt(buf, idx) {
  const text = buf.toString('utf8');
  // map byte offset to char offset
  const prefix = buf.subarray(0, idx).toString('utf8');
  const cidx = prefix.length;
  const sf = ts.createSourceFile('x.js', text, ts.ScriptTarget.ES2022, true, ts.ScriptKind.JS);
  for (const st of sf.statements) {
    const s = st.getStart(sf);
    if (s === cidx) {
      const endChar = st.end;
      return Buffer.byteLength(text.slice(0, endChar), 'utf8');
    }
  }
  return null;
}
for (const r of rows) {
  for (const c of r.citations) {
    const rec = { package: r.package, export: r.export, domain: r.domain, archive_path: c.archive_path, rc9: { start: c.start_byte, end: c.end_byte, slice: c.slice_sha256 } };
    const f9 = fs.readFileSync(R9 + PKGDIR[r.package] + '/' + c.archive_path);
    rec.rc9.file_ok = sha(f9) === c.file_sha256 && man('rc9', r.package)[c.archive_path]?.sha256 === c.file_sha256;
    const s9 = f9.subarray(c.start_byte, c.end_byte);
    rec.rc9.slice_ok = sha(s9) === c.slice_sha256;
    const t9 = s9.toString('utf8');
    rec.conv = { lineStart: c.start_byte === 0 || f9[c.start_byte - 1] === 0x0a, endsNL: t9.endsWith('\n') };
    const m = t9.match(/^(?:\s*\*\/\s*)?(?:function ([\w$]+)\(|const ([\w$]+) = )/);
    rec.name = m ? (m[1] || m[2]) : null;
    rec.kind = m ? (m[1] ? 'function' : 'const') : null;
    rec.rc9.defOffset = c.start_byte + (m ? t9.indexOf(m[1] ? 'function' : 'const') : 0);
    rec.rc9.prefix = t9.slice(0, rec.rc9.defOffset - c.start_byte);
    // rc.13
    const p13 = R13 + PKGDIR[r.package] + '/' + c.archive_path;
    if (!fs.existsSync(p13)) { rec.rc13 = { missing: true }; out.push(rec); continue; }
    const f13 = fs.readFileSync(p13);
    const m13 = man('rc13', r.package)[c.archive_path];
    const hits = rec.name ? findDef(f13, rec.name) : [];
    rec.rc13 = { file_sha256: sha(f13), manifest_ok: m13?.sha256 === sha(f13), hits: hits.length };
    if (hits.length === 1) {
      const def = hits[0]; // byte offset (latin1 index == byte index)
      const stEnd = stmtAt(f13, def);
      // also check rc.9 statement end for convention validation
      const stEnd9 = stmtAt(f9, rec.rc9.defOffset);
      rec.rc9.stmtEnd = stEnd9;
      let start = def;
      if (rec.conv.lineStart) { start = f13.lastIndexOf(0x0a, def - 1) + 1; }
      let end = stEnd;
      if (rec.conv.endsNL) { end = f13.indexOf(0x0a, stEnd - 1) + 1; }
      // sanity: rc.9 convention reproduces rc.9 slice?
      let s9s = rec.rc9.defOffset; if (rec.conv.lineStart) s9s = f9.lastIndexOf(0x0a, rec.rc9.defOffset - 1) + 1;
      let s9e = stEnd9; if (rec.conv.endsNL) s9e = f9.indexOf(0x0a, stEnd9 - 1) + 1;
      rec.rc9.reproduced = s9s === c.start_byte && s9e === c.end_byte;
      const s13 = f13.subarray(start, end);
      rec.rc13.start = start; rec.rc13.end = end; rec.rc13.slice_sha256 = sha(s13);
      rec.rc13.prefix = f13.subarray(start, def).toString('utf8');
      rec.rc13.identical = rec.rc13.slice_sha256 === c.slice_sha256;
    }
    out.push(rec);
  }
}
fs.writeFileSync(REPO + '/rust/target/audit-rc13/rows/tools/located.json', JSON.stringify(out, null, 1));
for (const o of out) console.log([o.package, o.export, o.domain, o.archive_path, o.name, o.rc9.file_ok && o.rc9.slice_ok ? 'rc9ok' : 'RC9BAD', o.rc9.reproduced ? 'conv-ok' : 'CONV?', JSON.stringify(o.conv), o.rc13.missing ? 'MISSING' : `hits=${o.rc13.hits} man=${o.rc13.manifest_ok} ${o.rc13.start}-${o.rc13.end} ${o.rc13.identical ? 'IDENTICAL' : 'changed'}`].join(' | '));
