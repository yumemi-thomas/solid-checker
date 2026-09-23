// What the compiled-in accepted-contract tier gives a consumer's call site.
//
// Read-only. It certifies nothing and runs no checker. It reads three pinned
// inputs and prints every number
// `2026-09-23-the-contract-story-assessed.md` cites:
//
//   pkg/contracts/accepted/                  the tier this build compiles in
//   docs/package-contract-v2/phase21/2026-09-14-consumer-demand-recensus.json
//                                            the frozen demand rows
//   benchmarks/ecosystem/report.json         the pinned ecosystem run
//
//     bun docs/package-contract-v2/phase22/2026-09-23-what-a-consumer-can-use.mjs
//
// The coverage census (`scripts/contract-coverage-census.mjs`) files a summary
// as determined when it states one operation or closes one claim domain. This
// asks the consumer's question instead: which claim domains does an import
// still find open, and does that open domain raise SC9005 wherever the name is
// imported or only on some uses. The answer mirrors
// `push_unknown_contract_claims` (rust/crates/solid-reactive-ir/src/contracts.rs),
// which reports at the import binding, and the callback-argument obligation in
// interproc.rs, which reports at the argument:
//
//   reads open     -> SC9005 at every import (`reads_completeness_demanded`)
//   creates open   -> SC9005 at every import (`ownerRequirements`)
//   returns open   -> SC9005 at the import unless every reference to the
//                     binding is a call whose result is discarded
//                     (`returns_shed_symbols`)
//   callbacks open -> SC9005 at each call passing a callable argument
//
// and `project_export_semantics`, which closes every call-path domain of a
// proven non-callable value export, since such an export is never invoked.
//
// Two approximations, both in the direction of overstating what a consumer
// gets: a domain closed on the wire is counted closed although projection can
// still reopen it for an item it cannot express, and the demand join takes the
// strongest answer across every nameable entrypoint exactly as the census
// does.
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";

import {
  census,
  nameableEntrypoint,
  summaryState,
  surfacesFromDocuments
} from "../../../scripts/contract-coverage-census.mjs";

const root = resolve(import.meta.dirname, "../../..");
const TIER = join(root, "pkg/contracts/accepted");
const DEMAND = join(root, "docs/package-contract-v2/phase21/2026-09-14-consumer-demand-recensus.json");
const REPORT = join(root, "benchmarks/ecosystem/report.json");

const CONSUMER_DOMAINS = ["callbacks", "reads", "returns", "creates"];
const EVERY_IMPORT = ["reads", "creates"];

const index = JSON.parse(readFileSync(join(TIER, "index.json"), "utf8"));
const documents = index.bundles.map(bundle => ({
  bundle,
  document: JSON.parse(readFileSync(join(TIER, bundle.document), "utf8"))
}));

/// A wire shape the projection proves non-callable. Mirrors
/// `shape_may_be_callable`: `callable`, `component` and `unknown` may be
/// invoked, and any structured shape is treated as callable here, which only
/// ever undercounts the values.
function provenValue(summary) {
  return typeof summary?.shape === "string" && !["callable", "component", "unknown"].includes(summary.shape);
}

/// The consumer's view of one summary: `value`, or the open consumer domains
/// of a callable and whether any of them fires at every import.
function consumerView(summary) {
  if (provenValue(summary)) return { state: "value", open: [] };
  const closed = new Set(summary?.call?.closed ?? []);
  const open = CONSUMER_DOMAINS.filter(domain => !closed.has(domain));
  if (open.length === 0) return { state: "clean", open };
  if (open.some(domain => EVERY_IMPORT.includes(domain))) return { state: "every-import", open };
  return { state: "some-uses", open };
}

const CONSUMER_RANK = { value: 4, clean: 3, "some-uses": 2, "every-import": 1 };

function add(counter, key, amount = 1) {
  counter[key] = (counter[key] ?? 0) + amount;
}

function share(part, whole) {
  return `${part} of ${whole} (${whole === 0 ? "0.0" : ((100 * part) / whole).toFixed(1)}%)`;
}

// --- 1. The tier, export by export ------------------------------------------

const exportsSeen = new Set();
const tier = {
  bundles: index.bundles.length,
  packages: new Set(index.bundles.map(bundle => bundle.packageName)).size,
  exports: 0,
  census: {},
  consumer: {},
  callablesByClosedConsumerDomains: {},
  callablesClosing: {},
  operationKinds: {},
  ownerRequirementExports: 0,
  invokeRows: {},
  returnShapes: {}
};
const perPackage = new Map();

for (const { bundle, document } of documents) {
  for (const [entrypoint, { cases }] of Object.entries(document.entrypoints ?? {})) {
    for (const artifactCase of cases ?? []) {
      for (const [name, reference] of Object.entries(artifactCase.exports ?? {})) {
        const key = [bundle.packageName, bundle.packageVersion, entrypoint, name, artifactCase.artifact?.sha256].join("|");
        if (exportsSeen.has(key)) continue;
        exportsSeen.add(key);
        const id = typeof reference === "string" ? reference : reference?.summary;
        const summary = document.summaries?.[id];
        const { state, owner } = summaryState(document, reference);
        const view = consumerView(summary);
        tier.exports += 1;
        add(tier.census, state);
        add(tier.consumer, view.state);
        const row = perPackage.get(bundle.packageName) ?? { exports: 0, degenerate: 0 };
        row.exports += 1;
        if (state === "degenerate") row.degenerate += 1;
        perPackage.set(bundle.packageName, row);
        if (view.state !== "value") {
          add(tier.callablesByClosedConsumerDomains, CONSUMER_DOMAINS.length - view.open.length);
          for (const domain of CONSUMER_DOMAINS) if (!view.open.includes(domain)) add(tier.callablesClosing, domain);
        }
        if (owner) tier.ownerRequirementExports += 1;
        const kinds = new Set();
        for (const operation of summary?.call?.operations ?? []) {
          kinds.add(operation.kind);
          if (operation.kind === "invoke") {
            add(tier.invokeRows, `${operation.at?.schedule ?? "no-schedule"} + ${operation.tracking ?? "no-tracking"}`);
          }
          if (operation.kind === "return") {
            add(tier.returnShapes, [operation.output?.kind, operation.output?.role].filter(Boolean).join(" "));
          }
        }
        for (const kind of kinds) add(tier.operationKinds, kind);
      }
    }
  }
}

// --- 2. The frozen demand, against the tier ---------------------------------

const demandRows = JSON.parse(readFileSync(DEMAND, "utf8")).rows;
const surfaces = surfacesFromDocuments(documents.map(({ document }) => document));
const reproduced = census(demandRows, surfaces).totals;

const consumerSurfaces = new Map();
for (const { document } of documents) {
  const name = document.package?.name;
  const surface = consumerSurfaces.get(name) ?? new Map();
  consumerSurfaces.set(name, surface);
  for (const [entrypoint, { cases }] of Object.entries(document.entrypoints ?? {})) {
    if (!nameableEntrypoint(entrypoint)) continue;
    for (const artifactCase of cases ?? []) {
      for (const [exportName, reference] of Object.entries(artifactCase.exports ?? {})) {
        const id = typeof reference === "string" ? reference : reference?.summary;
        const summary = document.summaries?.[id];
        const view = { ...consumerView(summary), census: summaryState(document, reference).state };
        const held = surface.get(exportName);
        if (!held || CONSUMER_RANK[view.state] > CONSUMER_RANK[held.state]) surface.set(exportName, view);
      }
    }
  }
}

const demand = {
  inSurface: 0,
  byConsumer: {},
  openOnEveryImport: {},
  openOnSomeUses: {},
  valuesTheCensusCallsDegenerate: 0,
  callablesTheCensusCallsDetermined: 0
};
for (const row of demandRows) {
  const view = consumerSurfaces.get(row.package)?.get(row.export);
  if (!view) continue;
  demand.inSurface += row.sites;
  add(demand.byConsumer, view.state, row.sites);
  if (view.state === "every-import") {
    for (const domain of view.open) add(demand.openOnEveryImport, domain, row.sites);
    add(demand.openOnEveryImport, `census ${view.census}`, row.sites);
  }
  if (view.state === "some-uses") for (const domain of view.open) add(demand.openOnSomeUses, domain, row.sites);
  if (view.state === "value" && view.census === "degenerate") demand.valuesTheCensusCallsDegenerate += row.sites;
  if (view.state !== "value" && view.census !== "degenerate") demand.callablesTheCensusCallsDetermined += row.sites;
}

// --- 3. The pinned ecosystem run's Solid 2 half -----------------------------

const report = JSON.parse(readFileSync(REPORT, "utf8"));
const solid2 = report.solid2.totals;
const content = report.solid2.families
  .flatMap(family => family.results)
  .map(result => result.contractContent)
  .filter(summary => summary?.measured);
const unknownByDomain = {};
for (const summary of content) for (const [domain, count] of Object.entries(summary.unknownByDomain ?? {})) add(unknownByDomain, domain, count);
const exportsTotal = content.reduce((sum, summary) => sum + (summary.exportsTotal ?? 0), 0);
const exportsProven = content.reduce((sum, summary) => sum + (summary.exportsProven ?? 0), 0);

// --- Print -------------------------------------------------------------------

const lines = [];
lines.push(`tier: ${tier.bundles} bundles, ${tier.packages} packages, ${tier.exports} exports`);
lines.push(`  census view:   ${JSON.stringify(tier.census)}`);
lines.push(`  consumer view: ${JSON.stringify(tier.consumer)}`);
lines.push(`  degenerate:    ${share(tier.census.degenerate ?? 0, tier.exports)}`);
lines.push(`  callables by consumer domains closed (0-4): ${JSON.stringify(tier.callablesByClosedConsumerDomains)}`);
lines.push(`  callables closing each consumer domain:     ${JSON.stringify(tier.callablesClosing)}`);
lines.push(`  exports stating an operation of each kind:  ${JSON.stringify(tier.operationKinds)}`);
lines.push(`  exports carrying an owner requirement:      ${tier.ownerRequirementExports}`);
lines.push(`  invoke rows by schedule + tracking:         ${JSON.stringify(tier.invokeRows)}`);
lines.push(`  return rows by returned shape:              ${JSON.stringify(tier.returnShapes)}`);
for (const [name, row] of [...perPackage].sort((left, right) => right[1].exports - left[1].exports)) {
  lines.push(`  ${name.padEnd(30)} ${String(row.exports).padStart(5)} exports, ${String(row.degenerate).padStart(5)} degenerate`);
}
lines.push("");
lines.push(`demand: ${demandRows.reduce((sum, row) => sum + row.sites, 0)} in-corpus sites`);
lines.push(`  census reproduced over this tier: ${JSON.stringify(reproduced)}`);
lines.push(`  in-surface sites: ${demand.inSurface}`);
lines.push(`  consumer view:    ${JSON.stringify(demand.byConsumer)}`);
lines.push(`  no SC9005 from any use (values): ${share(demand.byConsumer.value ?? 0, demand.inSurface)}`);
lines.push(`  SC9005 at every import:          ${share(demand.byConsumer["every-import"] ?? 0, demand.inSurface)}`);
lines.push(`  domains open on those:           ${JSON.stringify(demand.openOnEveryImport)}`);
lines.push(`  SC9005 on some uses only:        ${share(demand.byConsumer["some-uses"] ?? 0, demand.inSurface)}`);
lines.push(`  domains open on those:           ${JSON.stringify(demand.openOnSomeUses)}`);
lines.push(`  callables the census calls determined: ${demand.callablesTheCensusCallsDetermined}`);
lines.push(`  values the census calls degenerate:    ${demand.valuesTheCensusCallsDegenerate}`);
lines.push("");
lines.push(`report ${report.finishedAt}, solid2:`);
lines.push(`  ${solid2.compatiblePackageCount} packages, ${solid2.probeCount} probes`);
lines.push(`  certification: ${JSON.stringify({ ...solid2.certification, lanes: undefined })}`);
lines.push(`  exports ${exportsTotal}, proven ${exportsProven}, unknown by domain ${JSON.stringify(unknownByDomain)}`);
console.log(lines.join("\n"));
