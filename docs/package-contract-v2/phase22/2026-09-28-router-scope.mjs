// The measurement behind 2026-09-28-router-scope.md: which walls hold the
// `@solidjs/router` exports that real Solid 2 apps import.
//
// Read-only: it certifies nothing and runs no checker. Three modes:
//
//   bun docs/package-contract-v2/phase22/2026-09-28-router-scope.mjs --prepare <dir> [--no-scratch]
//       writes <dir>/manifest.json (the benchmark manifest, whose router rows
//       are next.30, next.26 on the rc.9 triple and next.18 on the rc.3 triple,
//       plus a scratch next.21 row on the rc.6 triple its apps install) and
//       <dir>/corpus.json (a certification-metric corpus naming each router
//       row; `--no-scratch` leaves next.21 out of both). Feed them to
//       `scripts/ecosystem-benchmark/run.mjs --manifest` with the
//       certification-metric flags, then `scripts/certification-metric.mjs
//       --run … --corpus <dir>/corpus.json`.
//
//   bun … --closures <tarballs> --sites <app-import metric.json> --json <out.json>
//       <tarballs>/next.N/package is `npm pack @solidjs/router@2.0.0-next.N`
//       unpacked. For each demanded export, digests its closure inside its
//       entry file (`default` condition: "." is dist/index.js, a bundle;
//       "./fs" is dist/fs.js): the export's top-level declaration plus every
//       top-level declaration of that file whose name a collected declaration
//       mentions, and the imported names it reaches. Over-approximate by
//       construction (any identifier naming a top-level binding counts), so an
//       equal digest is a strong statement and a different digest a weak one.
//
//   bun … --join --sites <app-import metric.json> --closures <json>
//            --metric <dir> [--hosts "none browser node"] [--exact]
//            [--measured 2.0.0-next.30,2.0.0-next.26,…]
//       joins app sites to per-host certification-metric exports
//       (<dir>/metric{,-browser,-node}.json) and prints the walls table and the
//       greedy unlock curve. A site at a measured version reads that version's
//       record. A site at an unmeasured version reads the record of a measured
//       version whose closure digest for that export is identical (nearest such
//       version first), else the nearest measured version (counted as
//       `nearest`). Both are estimates: a case-wide hazard follows the whole
//       entry file, not the export's closure. `--exact` keeps measured versions only.
import { createRequire } from "node:module";
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { createHash } from "node:crypto";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const argv = process.argv.slice(2);
const arg = name => { const i = argv.indexOf(name); return i < 0 ? undefined : argv[i + 1]; };

// next.21 has no committed row: its apps run rc.6, and `solid-js` and
// `@solidjs/web` rc.6 are not audited archives, so it is measured for its
// bytes only. Integrity: `npm view @solidjs/router@2.0.0-next.21
// dist.integrity`, 2026-09-28.
const SCRATCH = {
  version: "2.0.0-next.21",
  integrity: "sha512-sU9n6HsWpkHN0EmT5/r7z2uXzqlIWFJEJMe3V0MXCe19S4Gc8j33sSNZzrF431uOXwUQ+/KPKU4ChzcPQPDoEw==",
  peer: "^2.0.0-rc.5",
  solid: { "@solidjs/web": "2.0.0-rc.6", "solid-js": "2.0.0-rc.6", "@solidjs/signals": "2.0.0-rc.6" }
};

if (argv.includes("--prepare")) {
  // The manifest's own router rows (next.30, next.26 on rc.9, next.18 on
  // rc.3) plus the scratch next.21 row, and a certification-metric corpus
  // naming each of them.
  const out = resolve(arg("--prepare")); mkdirSync(out, { recursive: true });
  const manifest = JSON.parse(readFileSync(`${ROOT}/scripts/ecosystem-benchmark/manifest.json`, "utf8"));
  const routerRows = manifest.rows.filter(r => r.package === "@solidjs/router" && r.solidTarget === "solid2");
  const last = manifest.rows.indexOf(routerRows[routerRows.length - 1]);
  const { version, integrity, peer, solid } = SCRATCH;
  const scratch = {
    ...routerRows[0], version, integrity, distTags: [],
    peerDependencies: { "solid-js": peer, "@solidjs/web": peer },
    compatibleSolidVersions: Object.fromEntries(Object.entries(solid).map(([k, v]) => [k, [v]])),
    probes: [{ id: `@solidjs/router@${version}|solid2|only`, kind: "only", channel: "rc", solid }]
  };
  const withScratch = !argv.includes("--no-scratch");
  if (withScratch) manifest.rows.splice(last + 1, 0, scratch);
  writeFileSync(`${out}/manifest.json`, JSON.stringify(manifest, null, 2));
  const corpus = JSON.parse(readFileSync(`${ROOT}/scripts/ecosystem-benchmark/certification-metric-corpus.json`, "utf8"));
  const row = corpus.packages.find(p => p.package === "@solidjs/router");
  corpus.packages = [...routerRows, ...(withScratch ? [scratch] : [])].map((manifestRow, i) => ({
    ...row, rank: i === 0 ? row.rank : 1000 + i, version: manifestRow.version, integrity: manifestRow.integrity,
    probe: manifestRow.probes[0].id, solid: manifestRow.probes[0].solid
  }));
  corpus.selection.size = corpus.packages.length;
  writeFileSync(`${out}/corpus.json`, JSON.stringify(corpus, null, 2));
  console.log(`wrote ${out}/manifest.json and ${out}/corpus.json; probes:`);
  for (const entry of corpus.packages) console.log(`  ${entry.probe}`);
}

const VERSIONS = [16, 17, 18, 19, 20, 21, 23, 24, 26, 30].map(n => `next.${n}`);

if (argv.includes("--closures") && !argv.includes("--join")) {
  const require = createRequire(`${ROOT}/packages/cli/package.json`);
  const acorn = require("acorn");
  const tarballs = resolve(arg("--closures"));
  const demanded = [...new Set(JSON.parse(readFileSync(resolve(arg("--sites")), "utf8")).sites
    .filter(s => s.package === "@solidjs/router" && s.role === "app")
    .map(s => `${s.module === "@solidjs/router" ? "." : "." + s.module.slice("@solidjs/router".length)} ${s.export}`))]
    .sort().map(key => ({ entry: key.split(" ")[0], name: key.split(" ")[1] }));
  const cache = new Map();
  const analyse = file => {
    if (cache.has(file)) return cache.get(file);
    const src = readFileSync(file, "utf8");
    const ast = acorn.parse(src, { ecmaVersion: "latest", sourceType: "module" });
    const decls = new Map(), exportsMap = new Map(), importsMap = new Map();
    const names = p => !p ? [] : p.type === "Identifier" ? [p.name]
      : p.type === "ObjectPattern" ? p.properties.flatMap(q => names(q.value ?? q.argument))
      : p.type === "ArrayPattern" ? p.elements.flatMap(names)
      : p.type === "RestElement" ? names(p.argument)
      : p.type === "AssignmentPattern" ? names(p.left) : [];
    for (const node of ast.body) {
      const text = src.slice(node.start, node.end);
      if (node.type === "ImportDeclaration") {
        for (const s of node.specifiers) importsMap.set(s.local.name, `${node.source.value}#${s.imported ? (s.imported.name ?? s.imported.value) : "*"}`);
        continue;
      }
      let decl = node;
      if (node.type === "ExportNamedDeclaration") {
        if (!node.declaration) { for (const s of node.specifiers) exportsMap.set(s.exported.name ?? s.exported.value, s.local.name ?? s.local.value); continue; }
        decl = node.declaration;
      }
      const own = decl.type === "VariableDeclaration" ? decl.declarations.flatMap(d => names(d.id))
        : decl.id ? [decl.id.name] : [`<statement@${node.start}>`];
      for (const name of own) { decls.set(name, { text, node: decl }); if (decl !== node) exportsMap.set(name, name); }
    }
    const result = { decls, exportsMap, importsMap };
    cache.set(file, result);
    return result;
  };
  const identifiers = (node, out = new Set()) => {
    if (!node || typeof node.type !== "string") return out;
    if (node.type === "Identifier") out.add(node.name);
    for (const [key, v] of Object.entries(node)) {
      if (key === "start" || key === "end" || key === "type") continue;
      if (Array.isArray(v)) v.forEach(x => identifiers(x, out)); else if (v && typeof v.type === "string") identifiers(v, out);
    }
    return out;
  };
  const table = {};
  for (const { entry, name } of demanded) {
    const by = {};
    for (const v of VERSIONS) {
      const file = `${tarballs}/${v}/package/dist/${entry === "." ? "index.js" : entry.slice(2) + ".js"}`;
      const mod = existsSync(file) ? analyse(file) : null;
      const local = mod?.exportsMap.get(name);
      if (!local) { by[v] = "absent"; continue; }
      const seen = new Set(), external = new Set(), todo = [local];
      while (todo.length) {
        const n = todo.pop();
        if (seen.has(n)) continue;
        seen.add(n);
        if (mod.importsMap.has(n)) { external.add(mod.importsMap.get(n)); continue; }
        const d = mod.decls.get(n);
        if (d) for (const id of identifiers(d.node)) if (!seen.has(id) && (mod.decls.has(id) || mod.importsMap.has(id))) todo.push(id);
      }
      const texts = [...new Set([...seen].filter(n => mod.decls.has(n)).map(n => mod.decls.get(n).text))].sort();
      by[v] = createHash("sha256").update(texts.join("\n") + "\n--\n" + [...external].sort().join("\n")).digest("hex").slice(0, 12);
    }
    table[`${entry} ${name}`] = by;
    const letters = new Map();
    console.log([`${entry} ${name}`, ...VERSIONS.map(v => by[v] === "absent" ? "-" : (letters.has(by[v]) || letters.set(by[v], String.fromCharCode(65 + letters.size)), letters.get(by[v])))].join("\t"));
  }
  if (arg("--json")) writeFileSync(resolve(arg("--json")), JSON.stringify(table, null, 1));
}

if (argv.includes("--join")) {
  const app = JSON.parse(readFileSync(resolve(arg("--sites")), "utf8"));
  const closures = JSON.parse(readFileSync(resolve(arg("--closures")), "utf8"));
  const metricDir = resolve(arg("--metric"));
  const hosts = (arg("--hosts") ?? "none browser node").split(" ");
  const exactOnly = argv.includes("--exact");
  const measured = (arg("--measured") ?? "2.0.0-next.30,2.0.0-next.26,2.0.0-next.21").split(",");
  const demand = new Map();
  for (const s of app.sites) {
    if (s.package !== "@solidjs/router" || s.role !== "app" || !s.version.startsWith("2.0.0-next.")) continue;
    const entry = s.module === "@solidjs/router" ? "." : "." + s.module.slice("@solidjs/router".length);
    const key = `${entry} ${s.export}`;
    const d = demand.get(key) ?? new Map();
    d.set(s.version, (d.get(s.version) ?? 0) + s.sites);
    demand.set(key, d);
  }
  const stats = { exact: 0, identical: 0, nearest: 0 };
  const recordFor = (records, key, version) => {
    if (measured.includes(version)) { stats.exact++; return records[version]?.get(key); }
    const tag = version.slice("2.0.0-".length), by = closures[key];
    const n = Number(tag.split(".")[1]);
    // Nearest first: an export's own closure can be identical while the case
    // around it is not (next.30's dist/fs.js imports a peer next.26's does
    // not), and case-wide hazards follow the case.
    const order = measured
      .map(m => [m, Number(m.split(".").pop())])
      .sort((a, b) => Math.abs(a[1] - n) - Math.abs(b[1] - n) || b[1] - a[1])
      .map(([m]) => m);
    if (by && by[tag] && by[tag] !== "absent") {
      const same = order.find(m => by[m.slice("2.0.0-".length)] === by[tag]);
      if (same) { stats.identical++; return records[same]?.get(key); }
    }
    stats.nearest++;
    return records[order[0]]?.get(key);
  };
  for (const host of hosts) {
    const file = `${metricDir}/metric${host === "none" ? "" : "-" + host}.json`;
    if (!existsSync(file)) continue;
    const records = {};
    for (const p of JSON.parse(readFileSync(file, "utf8")).packages) records[p.version] = new Map(p.exports.map(e => [`${e.entrypoint} ${e.export}`, e]));
    const items = []; let absent = 0;
    for (const [key, byVersion] of demand) for (const [version, sites] of byVersion) {
      if (exactOnly && !measured.includes(version)) continue;
      const e = recordFor(records, key, version);
      if (!e) { absent += sites; continue; }
      items.push({ key, sites, clean: e.bucket === "clean", walls: new Set(e.causes.map(c => `${c.class}: ${c.key}`)) });
    }
    const total = items.reduce((a, x) => a + x.sites, 0);
    console.log(`\n## host ${host}: ${total} sites at an exported name (${absent} at a name the version does not export), ${items.filter(x => x.clean).reduce((a, x) => a + x.sites, 0)} clean`);
    const walls = new Map();
    for (const it of items) for (const w of it.walls) {
      const r = walls.get(w) ?? { sites: 0, solely: 0, exports: new Map() };
      r.sites += it.sites; if (it.walls.size === 1) r.solely += it.sites;
      r.exports.set(it.key, (r.exports.get(it.key) ?? 0) + it.sites);
      walls.set(w, r);
    }
    console.log("\n| wall | sites blocked | solely | top exports |\n| --- | ---: | ---: | --- |");
    for (const [w, r] of [...walls].sort((a, b) => b[1].sites - a[1].sites)) {
      console.log(`| ${w} | ${r.sites} | ${r.solely} | ${[...r.exports].sort((a, b) => b[1] - a[1]).slice(0, 4).map(([k, n]) => `\`${k.replace(/^\. /, "")}\` ${n}`).join(", ")} |`);
    }
    // Greedy unlock curve: repeatedly close the wall set that clears the most
    // sites per wall added.
    const closed = new Set(), done = new Set(); let cleared = 0;
    console.log("\nunlock curve:");
    for (;;) {
      let best = null;
      for (const it of items) {
        if (done.has(it) || it.clean) continue;
        const add = [...it.walls].filter(w => !closed.has(w));
        const trial = new Set([...closed, ...add]);
        const gain = items.filter(x => !done.has(x) && !x.clean && [...x.walls].every(w => trial.has(w))).reduce((a, x) => a + x.sites, 0);
        const score = gain / Math.max(add.length, 0.5);
        if (!best || score > best.score) best = { add, gain, score };
      }
      if (!best || best.gain === 0) break;
      best.add.forEach(w => closed.add(w));
      const newly = items.filter(x => !done.has(x) && !x.clean && [...x.walls].every(w => closed.has(w)));
      newly.forEach(x => done.add(x)); cleared += best.gain;
      const byKey = {}; for (const x of newly) byKey[x.key] = (byKey[x.key] ?? 0) + x.sites;
      console.log(`  +${best.gain} (${cleared}) closing ${best.add.join(" + ")}: ${Object.entries(byKey).map(([k, n]) => `${k} ${n}`).join(", ")}`);
    }
  }
  console.log(`\nrecords read: ${JSON.stringify(stats)}`);
}
