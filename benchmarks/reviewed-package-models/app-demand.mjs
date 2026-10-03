// A demand/admission audit of retained app observations. This does not run the
// analyzer on apps or claim their candidate premises establish a defect.
import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { authenticateModel, hash, read } from "./catalog.mjs";
const [metricArgument, corpusArgument, catalogArgument, outputArgument] = process.argv.slice(2);
assert(outputArgument, "Usage: node app-demand.mjs <metric.json> <app-corpus.json> <catalog.json> <fresh-output.json>");
const metricPath = resolve(metricArgument), metric = read(metricPath), corpus = read(resolve(corpusArgument)), catalog = read(resolve(catalogArgument));
const output = resolve(outputArgument); assert(!existsSync(output), "Output already exists");
const rows = metric.sites.filter(row => row.role === "app" && row.origin === "third-party");
const appRoot = join(dirname(metricPath), "apps"), memo = new Map(), packages = new Map(), classified = [];
for (const row of rows) {
  const model = catalog.models.find(item => item.package === row.package);
  const behavior = model?.exports[row.export]?.browser;
  let admission = "package unmodeled", detail = null;
  if (model) {
    if (model.version !== row.version) admission = "version mismatch";
    else if (row.module !== row.package) admission = "subpath unmodeled";
    else if (!behavior) admission = "export has no browser premise";
    else {
      const key = `${row.app}:${row.project}:${row.package}`;
      if (!memo.has(key)) {
        const app = corpus.apps.find(item => item.id === row.app);
        try {
          assert(app, "App absent from pinned corpus");
          // Corpus projects are repository-relative, including the app folder.
          const project = join(appRoot, row.app, row.project);
          assert(existsSync(project), "Retained app project missing");
          authenticateModel(model, dirname(project)); memo.set(key, { admission: "pins match", detail: null });
        } catch (error) { memo.set(key, { admission: "input mismatch", detail: error.message }); }
      }
      ({ admission, detail } = memo.get(key));
    }
  }
  const item = { app: row.app, package: row.package, version: row.version, module: row.module, export: row.export, sites: row.sites,
    nameHasPremise: Boolean(behavior), admission, detail };
  classified.push(item);
  const demand = packages.get(row.package) ?? { package: row.package, sites: 0, nameHasPremise: 0, pinsMatch: 0 };
  demand.sites += row.sites; demand.nameHasPremise += behavior ? row.sites : 0; demand.pinsMatch += admission === "pins match" ? row.sites : 0;
  packages.set(row.package, demand);
}
const total = items => items.reduce((sum, item) => sum + item.sites, 0), states = {};
for (const row of classified) states[row.admission] = (states[row.admission] ?? 0) + row.sites;
const report = { authority: false, kind: "historical-demand-and-input-admission-only", metricPath, metricGeneratedAt: metric.generatedAt,
  metricSha256: hash(readFileSync(metricPath)), corpusSha256: hash(readFileSync(resolve(corpusArgument))), catalogSha256: hash(readFileSync(resolve(catalogArgument))),
  summary: { apps: metric.headline.apps, thirdPartySites: total(rows), primitiveSites: total(rows.filter(row => row.package.startsWith("@solid-primitives/"))),
    sitesInModelPackages: total(rows.filter(row => catalog.models.some(model => model.package === row.package))),
    nameHasPremise: total(classified.filter(row => row.nameHasPremise)), pinsMatch: total(classified.filter(row => row.admission === "pins match")), states },
  packages: [...packages.values()].sort((a, b) => b.sites - a.sites), sites: classified };
writeFileSync(output, JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify(report.summary, null, 2));
