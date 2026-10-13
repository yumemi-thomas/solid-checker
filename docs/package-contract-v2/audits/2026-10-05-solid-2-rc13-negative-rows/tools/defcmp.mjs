// Compare named top-level definitions of one file between rc9 and rc13 (sha16), and
// optionally across sibling files. usage: node defcmp.mjs <pkg> <rel,rel,...> name...
import crypto from 'node:crypto';
import { topDefs } from './topwalk.mjs';
const [,, pkg, rels, ...names] = process.argv;
const h = (t) => t ? crypto.createHash('sha256').update(t).digest('hex').slice(0, 12) : '-';
for (const n of names) {
  const row = [n];
  for (const rel of rels.split(',')) {
    const a = topDefs('rc9', pkg, rel).defs.get(n), b = topDefs('rc13', pkg, rel).defs.get(n);
    row.push(`${rel.replace('dist/', '')}: ${h(a?.text)}→${h(b?.text)}${a && b && a.text === b.text ? ' =' : ' ≠'}`);
  }
  console.log(row.join(' | '));
}
