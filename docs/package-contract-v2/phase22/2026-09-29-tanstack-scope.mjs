// The measurement behind 2026-09-29-tanstack-scope.md: which walls hold the
// `@tanstack/solid-router` and `@tanstack/solid-query` exports that real
// Solid 2 apps import. The method is 2026-09-28-router-scope.mjs's, with one
// difference: every installed version is certified here (on the audited rc.9
// triple), so every app site reads its own version's record and no closure
// digest stands in for an unmeasured version.
//
// Read-only: it certifies nothing and runs no checker. Two modes:
//
//   bun docs/package-contract-v2/phase22/2026-09-29-tanstack-scope.mjs --prepare <dir>
//       writes <dir>/manifest.json (the benchmark manifest plus a scratch
//       `solid2` row per installed version the manifest does not carry, each
//       with one probe on the rc.9 triple) and <dir>/corpus.json (a
//       certification-metric corpus naming one probe per installed version).
//       Feed them to `scripts/ecosystem-benchmark/run.mjs --manifest` with the
//       certification-metric flags, then `scripts/certification-metric.mjs
//       --run … --corpus <dir>/corpus.json`.
//
//   bun … --join --sites <app-import metric.json> --metric <dir> [--hosts "none browser node"]
//       joins app sites to per-host certification-metric exports
//       (<dir>/metric{,-browser,-node}.json) and prints, per package, the
//       per-export record, the walls table and the greedy unlock curve.
//       `--certifiable` keeps only sites whose app runs an audited triple.
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const argv = process.argv.slice(2);
const arg = name => { const i = argv.indexOf(name); return i < 0 ? undefined : argv[i + 1]; };
const PACKAGES = ["@tanstack/solid-router", "@tanstack/solid-query"];
const HEAD = { "@solidjs/web": "2.0.0-rc.9", "solid-js": "2.0.0-rc.9" };
// The runtime triples with audited archives (rust/crates/solid-dialect/audited-archives.json).
const AUDITED = new Set(["2.0.0-rc.3", "2.0.0-rc.9"]);

// Installed versions the manifest has no row for, measured for their bytes on
// rc.9. Integrity and peers: `npm view <spec> dist.integrity peerDependencies`,
// 2026-09-29.
const SCRATCH = [
  { package: "@tanstack/solid-router", version: "2.0.0-rc.6", like: "2.0.0-rc.8",
    integrity: "sha512-ahTyk3ObuVQtcRFlWbDxe6glsz3KByYoqAYv4iWsLjmiXL+lw2fAuc67dOPQc6nt8AoOKIRy+/VLsMITw+GN8A==" },
  { package: "@tanstack/solid-router", version: "2.0.0-rc.4", like: "2.0.0-rc.8",
    integrity: "sha512-DCHpsdvlWuJ3064od5ScYe4fUaFFaj8z/cqDPggh5CJ2hI/ZjUrCa1GnHbbjWGNk/WKDLfwdwRDPHFKU/dI/ug==" },
  { package: "@tanstack/solid-router", version: "2.0.0-beta.29", like: "2.0.0-rc.8",
    integrity: "sha512-UAZhtkZkIRb+lH98FHIrVBBMFJN4vf89NRUYNoq0FYnJQywUQ0c9GlZL6yqhMF5Vzs8AnQ2hDGiLgYZUGu38gg==" },
  { package: "@tanstack/solid-query", version: "6.0.0-rc.3", like: "6.0.0-rc.4",
    integrity: "sha512-Wj23dMyRnVmKiYvvyMImZqb3+2Ftxc7FngUfloCAmQE2dpQO/QeqaSHNlpjbG6eoKqazVPe4k1xhxofUU+w/dA==",
    peerDependencies: { "solid-js": ">=2.0.0-rc.6 <3.0.0", "@solidjs/web": ">=2.0.0-rc.6 <3.0.0" } },
  { package: "@tanstack/solid-query", version: "6.0.0-rc.0", like: "6.0.0-rc.4",
    integrity: "sha512-v6U7OWYAI9DAPrmDjizCJ90/RrJU+F2Uvu9s/xfwsnOVHQeEC15T2Jsmn1dqCHm4XpuUv21t1SNgLqB+zzzsJQ==",
    peerDependencies: { "solid-js": ">=2.0.0-rc.0 <3.0.0" } }
];

const headProbe = row =>
  row.probes.find(probe => probe.kind === "head") ?? row.probes[row.probes.length - 1];

if (argv.includes("--prepare")) {
  const out = resolve(arg("--prepare")); mkdirSync(out, { recursive: true });
  const manifest = JSON.parse(readFileSync(`${ROOT}/scripts/ecosystem-benchmark/manifest.json`, "utf8"));
  const measured = [];
  for (const name of PACKAGES) {
    for (const row of manifest.rows.filter(r => r.package === name && r.solidTarget === "solid2")) {
      measured.push({ row, probe: headProbe(row) });
    }
  }
  for (const scratch of SCRATCH) {
    const like = manifest.rows.find(r => r.package === scratch.package && r.version === scratch.like && r.solidTarget === "solid2");
    const row = {
      ...like,
      version: scratch.version,
      integrity: scratch.integrity,
      distTags: [],
      ...(scratch.peerDependencies ? { peerDependencies: scratch.peerDependencies } : {}),
      compatibleSolidVersions: Object.fromEntries(Object.entries(HEAD).map(([k, v]) => [k, [v]])),
      probes: [{ id: `${scratch.package}@${scratch.version}|solid2|only`, kind: "only", channel: "rc", solid: HEAD }]
    };
    manifest.rows.splice(manifest.rows.indexOf(like) + 1, 0, row);
    measured.push({ row, probe: row.probes[0] });
  }
  writeFileSync(`${out}/manifest.json`, JSON.stringify(manifest, null, 2));
  const corpus = JSON.parse(readFileSync(`${ROOT}/scripts/ecosystem-benchmark/certification-metric-corpus.json`, "utf8"));
  const template = corpus.packages.find(p => p.package === "@tanstack/solid-query");
  corpus.packages = measured.map(({ row, probe }, i) => ({
    ...template, rank: 2000 + i, package: row.package, version: row.version, integrity: row.integrity,
    probe: probe.id, probeKind: probe.kind, solid: probe.solid
  }));
  corpus.selection.size = corpus.packages.length;
  writeFileSync(`${out}/corpus.json`, JSON.stringify(corpus, null, 2));
  console.log(`wrote ${out}/manifest.json and ${out}/corpus.json; probes:`);
  for (const entry of corpus.packages) console.log(`  ${entry.probe}`);
}

if (argv.includes("--join")) {
  const app = JSON.parse(readFileSync(resolve(arg("--sites")), "utf8"));
  const metricDir = resolve(arg("--metric"));
  const hosts = (arg("--hosts") ?? "none browser node").split(" ");
  const certifiableOnly = argv.includes("--certifiable");
  const runtimeOf = new Map(app.apps.map(a => [a.id, Object.values(a.solid ?? {})[0]?.["solid-js"]]));
  for (const name of PACKAGES) {
    const demand = [];
    for (const s of app.sites) {
      if (s.package !== name || s.role !== "app") continue;
      if (certifiableOnly && !AUDITED.has(runtimeOf.get(s.app))) continue;
      const entry = s.module === name ? "." : "." + s.module.slice(name.length);
      demand.push({ key: `${entry} ${s.export}`, version: s.version, sites: s.sites, app: s.app });
    }
    console.log(`\n# ${name}: ${demand.reduce((a, d) => a + d.sites, 0)} app sites${certifiableOnly ? " on an audited runtime" : ""}`);
    for (const host of hosts) {
      const file = `${metricDir}/metric${host === "none" ? "" : "-" + host}.json`;
      if (!existsSync(file)) continue;
      const records = {};
      for (const p of JSON.parse(readFileSync(file, "utf8")).packages) {
        if (p.package !== name) continue;
        records[p.version] = new Map(p.exports.map(e => [`${e.entrypoint} ${e.export}`, e]));
      }
      const items = []; let absent = 0; let unmeasured = 0; let uncertifiedEntry = 0;
      for (const d of demand) {
        if (!records[d.version]) { unmeasured += d.sites; continue; }
        const e = records[d.version].get(d.key);
        const entry = d.key.split(" ")[0];
        if (!e && ![...records[d.version].keys()].some(k => k.split(" ")[0] === entry)) { uncertifiedEntry += d.sites; continue; }
        if (!e) { absent += d.sites; continue; }
        items.push({ ...d, clean: e.bucket === "clean", walls: new Set(e.causes.map(c => `${c.domain} ${c.class}: ${c.key}`)), causes: e.causes });
      }
      const total = items.reduce((a, x) => a + x.sites, 0);
      console.log(`\n## host ${host}: ${total} sites at an exported name, ${absent} at a name the version does not export, ${uncertifiedEntry} at an entrypoint this host did not certify, ${unmeasured} at an unmeasured version, ${items.filter(x => x.clean).reduce((a, x) => a + x.sites, 0)} clean`);
      const byExport = new Map();
      for (const it of items) {
        const r = byExport.get(it.key) ?? { sites: 0, versions: new Map() };
        r.sites += it.sites;
        r.versions.set(it.version, it.causes);
        byExport.set(it.key, r);
      }
      console.log("\n| export | sites | callbacks | reads | returns | creates |\n| --- | ---: | --- | --- | --- | --- |");
      for (const [key, r] of [...byExport].sort((a, b) => b[1].sites - a[1].sites)) {
        const [version, causes] = [...r.versions].sort((a, b) => (b[0] === "2.0.0-rc.8") - (a[0] === "2.0.0-rc.8"))[0];
        const cell = domain => causes.filter(c => c.domain === domain).map(c => `${c.class}: ${c.key}`).join("; ") || "closed";
        console.log(`| \`${key.replace(/^\. /, "")}\` (${version}) | ${r.sites} | ${cell("callbacks")} | ${cell("reads")} | ${cell("returns")} | ${cell("creates")} |`);
      }
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
  }
}
