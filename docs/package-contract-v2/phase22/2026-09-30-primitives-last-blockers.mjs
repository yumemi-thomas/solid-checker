#!/usr/bin/env bun
// Ranks what is the LAST blocker of a not-clean @solid-primitives export.
//
//   bun 2026-09-30-primitives-last-blockers.mjs measure.json measure-browser.json measure-node.json
//
// A cause is (domain, key) as `primitives-checkpoint.mjs --measure` records it.
// An export is blocked by the set of distinct causes it carries; it becomes
// clean only when the whole set closes. "Sole" counts exports whose set is
// exactly that cause, "pair" counts exports whose set is exactly those two.
import { readFileSync } from 'node:fs';

const files = process.argv.slice(2);
if (files.length === 0) {
  console.error('usage: last-blockers.mjs <measure.json>...');
  process.exit(2);
}
const top = Number(process.env.TOP ?? 12);
const out = [];
for (const file of files) {
  const m = JSON.parse(readFileSync(file, 'utf8'));
  const sole = new Map();
  const pair = new Map();
  const size = new Map();
  let clean = 0;
  let total = 0;
  for (const p of m.packages) {
    for (const e of p.exports ?? []) {
      total++;
      const label = (c) => {
        const domain = c.domain ?? 'package';
        return c.key.startsWith(`${domain}:`) ? c.key : `${domain}: ${c.key}`;
      };
      const set = [...new Set(e.causes.map(label))].sort();
      if (e.bucket === 'clean' || set.length === 0) {
        clean++;
        continue;
      }
      size.set(set.length, (size.get(set.length) ?? 0) + 1);
      const bump = (map, k, id) => {
        const v = map.get(k) ?? { n: 0, ex: [] };
        v.n++;
        if (v.ex.length < 3) v.ex.push(id);
        map.set(k, v);
      };
      const id = `${p.package.replace('@solid-primitives/', '')}:${e.export}`;
      if (set.length === 1) bump(sole, set[0], id);
      if (set.length === 2) bump(pair, set.join('  +  '), id);
    }
  }
  const rank = (map) => [...map].sort((a, b) => b[1].n - a[1].n).slice(0, top);
  out.push({ host: m.host, total, clean, size: Object.fromEntries([...size].sort((a, b) => a[0] - b[0])), sole: rank(sole), pair: rank(pair) });
}
for (const r of out) {
  console.log(`\n## host ${r.host}: clean ${r.clean} of ${r.total}; open exports by number of distinct causes ${JSON.stringify(r.size)}\n`);
  console.log('Sole blocker (export carries only this cause)\n');
  console.log('| exports | cause | examples |\n| ---: | --- | --- |');
  for (const [k, v] of r.sole) console.log(`| ${v.n} | ${k} | ${v.ex.join(', ')} |`);
  console.log('\nPairs (export carries exactly these two causes)\n');
  console.log('| exports | causes | examples |\n| ---: | --- | --- |');
  for (const [k, v] of r.pair) console.log(`| ${v.n} | ${k} | ${v.ex.join(', ')} |`);
}
