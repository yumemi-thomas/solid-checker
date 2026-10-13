// Offline admission/extraction experiment against the retained app installs.
// Import counts remain historical; this script does not diagnose app defects.
import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { hash, read } from "./catalog.mjs";
import { installedCatalog } from "./demand-models.mjs";

const [metricArgument, corpusArgument, outputArgument] = process.argv.slice(2);
assert(outputArgument, "Usage: node installed-app-demand.mjs <metric.json> <corpus.json> <fresh-output.json>");
const metricPath = resolve(metricArgument), corpusPath = resolve(corpusArgument), output = resolve(outputArgument);
assert(!existsSync(output), "Evidence output already exists");
const metric = read(metricPath), corpus = read(corpusPath), started = performance.now();
const rows = metric.sites.filter(row => row.role === "app" && row.origin === "third-party");
const groups = new Map();
for (const row of rows) {
  const key = `${row.app}:${row.project}`;
  const group = groups.get(key) ?? { app: row.app, project: row.project, rows: [] };
  group.rows.push(row); groups.set(key, group);
}
const report = { authority: false, kind: "installed-source-demand-only", metricSha256: hash(readFileSync(metricPath)),
  corpusSha256: hash(readFileSync(corpusPath)), startedAt: new Date().toISOString(), projects: [], sites: [], summary: null };
for (const group of groups.values()) {
  assert(corpus.apps.some(app => app.id === group.app), "App absent from pinned corpus");
  const project = join(dirname(metricPath), "apps", group.app, group.project);
  assert(existsSync(project), "Retained app project missing");
  const roots = group.rows.filter(row => row.module === row.package);
  const catalog = installedCatalog(dirname(project), roots.map(row => ({ package: row.package, exports: [row.export] })), "browser");
  report.projects.push({ app: group.app, project: group.project, catalog });
  for (const row of group.rows) {
    const model = catalog.models.find(model => model.package === row.package);
    const packageObservation = catalog.packages.find(item => item.package === row.package);
    const behavior = model?.exports[row.export]?.browser;
    const admission = row.module !== row.package ? "subpath unsupported" : packageObservation?.error ? "input refused" :
      model?.version !== row.version ? "historical package version changed" : behavior ? "installed source has premise" : "no supported source premise";
    report.sites.push({ app: row.app, project: row.project, package: row.package, export: row.export, module: row.module,
      sites: row.sites, admission, behavior, detail: packageObservation?.error });
  }
  writeFileSync(output, JSON.stringify(report, null, 2) + "\n");
  console.log(`${group.app}/${group.project}: ${catalog.models.length} installed packages admitted, ${catalog.durationMs.toFixed(0)} ms`);
}
const states = {};
for (const row of report.sites) states[row.admission] = (states[row.admission] ?? 0) + row.sites;
const candidates = report.sites.filter(row => row.admission === "installed source has premise");
report.summary = { apps: new Set(rows.map(row => row.app)).size, thirdPartySites: rows.reduce((sum, row) => sum + row.sites, 0),
  projects: groups.size, admittedApps: new Set(report.projects.filter(item => item.catalog.models.length).map(item => item.app)).size,
  installedPackageModels: report.projects.reduce((sum, item) => sum + item.catalog.models.length, 0),
  packagesWithPremises: new Set(candidates.map(row => row.package)).size,
  sitesWithPremises: candidates.reduce((sum, row) => sum + row.sites, 0),
  primitiveSitesWithPremises: candidates.filter(row => row.package.startsWith("@solid-primitives/")).reduce((sum, row) => sum + row.sites, 0),
  states, durationMs: performance.now() - started };
report.finishedAt = new Date().toISOString();
writeFileSync(output, JSON.stringify(report, null, 2) + "\n"); console.log(JSON.stringify(report.summary, null, 2));
