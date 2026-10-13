#!/usr/bin/env bun

// The gate for the question this whole phase exists to answer: *does a
// consumer with contracts for the packages they import get useful feedback?*
//
// It was answered twice by hand, on 2026-09-15 and 2026-09-16, and the two
// answers disagreed by a factor of two for the same package -- 0% and 33.1% for
// `@kobalte/utils` -- because one classified against emitted *proposals* and
// the other against *certified catalogs*. A number that moves that far on an
// unstated choice of input is not a measurement, so this fixes both halves.
//
//   bun scripts/contract-coverage-census.mjs --run <run.json>   enforce the pin
//   bun scripts/contract-coverage-census.mjs --run <run.json> --json
//   bun scripts/contract-coverage-census.mjs --run <run.json> --update
//
// **Demand** is pinned: `docs/package-contract-v2/phase21/`
// `2026-09-14-consumer-demand-recensus.json`, an SC9005 sweep over 146 real
// consumer projects -- 159 (package, export) pairs and 1,958 call sites that
// name a package the corpus installs. It is an input, never recomputed here: a
// coverage number whose *denominator* moves with the same run that moves its
// numerator cannot regress visibly.
//
// **Supply** is a directory of certified catalogs, produced by a run that keeps
// them (`make contract-coverage-census`). Certification is where an entrypoint
// like `@kobalte/utils`' `.` first appears at all, so proposals are the wrong
// side of the question, and nothing here certifies anything itself: the slow
// half is separate so the measurement stays deterministic and cheap to re-run.
//
// # The denominator names one package major; the corpus may install another
//
// Demand was swept over consumers written against the Solid 1.x-era releases of
// these packages. A `--solid 2` run installs each package's *Solid 2* release,
// which for a package mid-rewrite is a different API.
// `@kobalte/utils@2.0.0-alpha.0` exports fifteen names and contains no
// `mergeDefaultProps` anywhere in the package; the frozen demand asks about that
// export 254 times. Those sites land in `absent`, and so do 32 other exports the
// 2.0 release dropped -- 722 of 942 sites for that package alone.
//
// So on a cross-major run `absent` is **not** a contract gap. It reads "the
// demanded export is not in this artifact's certified surface", and most of it
// is the export not existing in that major at all. ADR 0110 s 5 froze the
// denominator on the argument that demand is a fact about the ecosystem rather
// than about the dialect. That argument holds for *counting* demand. It does not
// make one major's demand answerable by another major's artifact, and a reader
// comparing a `solid1` pin to a `solid2` one will otherwise read ecosystem
// churn as a coverage regression. This is why the comparison is refused outright
// rather than annotated.
//
// # The one classification rule that is easy to get wrong
//
// An export counts only at an entrypoint a consumer **can name**.
// `policy2_artifact_acceptance_root` binds the entrypoint, so a summary
// published at `./src/dom.ts` -- reachable only because the package declares a
// `./*` wildcard -- does not answer `import { contains } from "@kobalte/utils"`.
// Crediting those inflated the 2026-09-15 census, and it is the difference
// between "this package states something" and "this package states something
// *to its consumers*".
//
// # Two views of one summary
//
// The census buckets ask what a summary *states*: an operation, a closed claim
// domain, or nothing. That measures certification, and it is the wrong question
// for a consumer, because one closed domain files a summary as determined while
// the import still finds the others open. The **consumer view** asks what the
// import finds open, by the analyzer's own rule rather than a new one:
// `push_unknown_contract_claims` (rust/crates/solid-reactive-ir/src/contracts.rs)
// raises SC9005 at the import binding for an open `reads` or `creates`, and for
// an open `returns` unless every reference discards the call's result, while
// interproc.rs raises it for an open `callbacks` only at a call that passes a
// callable argument. Measured over the shipped tier on 2026-09-23, not one
// callable closed all four: the census called 882 of 1,152 in-surface sites
// determined, and the only ones an import reached with nothing open were 158
// non-callable values, 18 of which the census had called degenerate
// (docs/package-contract-v2/phase22/2026-09-23-the-contract-story-assessed.md).
// Both views go into the pin, and a pin written before the consumer view
// existed is compared on the census buckets alone.

import { readFileSync, writeFileSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const DEMAND = join(
  root,
  "docs/package-contract-v2/phase21/2026-09-14-consumer-demand-recensus.json"
);
const PIN = join(root, "benchmarks/ecosystem/coverage-census.json");

/// Entrypoint spellings a consumer cannot write as an import. A declared
/// subpath (`./immutable`, `./config`) is nameable; a wildcard-reached source
/// file is not, and is the trap the header describes.
const WILDCARD_REACHED = /\.(?:ts|tsx|js|jsx|mjs|cjs|d\.ts)$/;

/// Best-answer ordering. A name published at two nameable entrypoints keeps the
/// strongest statement, because that is the one its consumer gets.
const RANK = { operations: 3, "closed-empty": 2, degenerate: 1 };
const STATES = ["operations", "closed-empty", "degenerate", "absent"];

/// The claim domains a consumer's import reads, and the two of them whose
/// openness raises SC9005 wherever the name is imported. See "Two views of one
/// summary" above for where the analyzer decides this.
const CONSUMER_DOMAINS = ["callbacks", "reads", "returns", "creates"];
const OPEN_AT_EVERY_IMPORT = ["reads", "creates"];

/// Best-answer ordering for the consumer view, on `RANK`'s argument.
const CONSUMER_RANK = { value: 4, clean: 3, "some-uses": 2, "every-import": 1 };
const CONSUMER_STATES = ["value", "clean", "some-uses", "every-import"];

function fail(message) {
  console.error(`contract-coverage-census: ${message}`);
  process.exit(1);
}

/// Which Solid corpus this run measured, read from the run's own scope rather
/// than from a flag, so a pin cannot claim a denominator the run did not use.
///
/// The benchmark records `scope.solidTargets` as the bare majors it was
/// restricted to (`["1"]`, `["2"]`); an unrestricted run measures both and has
/// no single denominator to pin.
export function censusTarget(run) {
  // A consumer-environment run certifies rows cloned into one consumer's
  // installed tree so the compiled-in tier can be delivered there (owner
  // decision, 2026-09-26). It is delivery only: its catalogs are not the
  // corpus the pinned denominator describes, and its yield is measured by
  // re-sweeping that consumer. Counting it here would move the pin with
  // certifications the census never selected.
  const environment = run?.scope?.consumerEnvironment;
  if (environment) {
    fail(
      `this run is a delivery run for consumer environment ${environment}, not a census run; ` +
        "measure it by re-sweeping that consumer, and pin the census from `make contract-coverage-census`"
    );
  }
  const targets = run?.scope?.solidTargets ?? [];
  if (targets.length !== 1) {
    fail(
      `this run measured ${targets.length === 0 ? "every" : targets.length} Solid target(s); ` +
        "the census pin records one denominator, so restrict the run with a single --solid"
    );
  }
  // A requested export-condition set decides which *bytes* each row is
  // certified about, not merely which rows ran, so a conditioned run answers a
  // different question with the same buckets. `--conditions node` makes
  // `@solid-primitives/platform`'s gates complete where the default set cannot
  // run them at all, and a pin that took those numbers as this baseline's
  // would read a change of artifact as a change of coverage -- the same
  // mistake the cross-major refusal above exists to prevent. Measure with it
  // freely; pin without it.
  // A host run (ADR 0140) is the same: every case carries the host condition.
  if (run?.scope?.host) {
    fail(
      `this run certified for the ${run.scope.host} host, so it is certified about different ` +
        "bytes than the pinned baseline; re-run without --host to pin"
    );
  }
  const conditions = run?.scope?.conditions ?? [];
  if (conditions.length) {
    fail(
      `this run requested export conditions [${conditions.join(", ")}], so it is certified ` +
        "about different bytes than the pinned baseline; re-run without --conditions to pin"
    );
  }
  return `solid${targets[0]}`;
}

function parseArguments(argv) {
  const options = { catalogs: null, run: null, json: false, update: false, printPackages: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--catalogs") {
      options.catalogs = argv[index + 1];
      index += 1;
    } else if (argument === "--run") {
      options.run = argv[index + 1];
      index += 1;
    } else if (argument === "--json") options.json = true;
    else if (argument === "--print-packages") options.printPackages = true;
    else if (argument === "--update") options.update = true;
    else if (argument === "-h" || argument === "--help") {
      console.log(readFileSync(new URL(import.meta.url), "utf8").split("\n\n")[0]);
      process.exit(0);
    } else fail(`unknown argument ${JSON.stringify(argument)}`);
  }
  if (!options.catalogs && !options.run && !options.printPackages) {
    fail("one of --run <run.json> or --catalogs <DIR> is required");
  }
  return options;
}

/// Every `*.main.json` under a tree, which is how a published catalog stores
/// the document a receipt binds. Depth is unbounded on purpose: the catalog
/// nests by case set, case and object digest, and none of those levels is this
/// script's business.
function publishedDocuments(directory) {
  const found = [];
  const walk = current => {
    let entries;
    try {
      entries = readdirSync(current, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      const path = join(current, entry.name);
      if (entry.isDirectory()) walk(path);
      else if (entry.name.endsWith(".main.json")) found.push(path);
    }
  };
  walk(directory);
  return found.sort();
}

/// What one summary states, and whether it carries an owner requirement.
///
/// `operations` is a positive statement about behaviour. `closed-empty` is the
/// *determined negative*: the census proved the domain has nothing in it, which
/// is a complete answer and not a gap. `degenerate` is neither -- nothing was
/// determined -- and is the state this gate exists to push down.
export function summaryState(document, reference) {
  const id = typeof reference === "string" ? reference : reference?.summary;
  const summary = document.summaries?.[id];
  if (!summary) return { state: "degenerate", owner: false };
  const call = summary.call ?? {};
  const operations = call.operations ?? [];
  const owner = operations.some(operation => {
    const relation = operation.owner ?? {};
    return relation.requires === "required" || relation.requiresCleanup === "required";
  });
  if (operations.length > 0) return { state: "operations", owner };
  if ((call.closed ?? []).length > 0) return { state: "closed-empty", owner };
  return { state: "degenerate", owner };
}

/// Whether a return output carries a reactive accessor anywhere inside it: the
/// output itself, an object property's value, or a tuple item.
function outputCarriesAccessor(output) {
  if (!output || typeof output !== "object") return false;
  if (output.kind === "reactive" && output.role === "accessor") return true;
  return [
    ...(output.properties ?? []).map(property => property?.value),
    ...(output.items ?? [])
  ].some(outputCarriesAccessor);
}

/// Which positive claims in one summary a misuse rule could consume, by class.
/// Reported, never gated (ways-to-improve § 3.1): the count says how much of
/// the certified surface can feed a violation at all, which the coverage
/// buckets cannot, since a closed domain with nothing in it feeds nothing.
///
/// - `owner`: an owner requirement (the missing-owner paths).
/// - `argumentRead`: a `read` whose input is a parameter of the call.
/// - `returnedAccessor`: a `return` whose output carries a reactive accessor.
/// - `invoke`: an `invoke` stating its tracking. Every shipped row states one,
///   and most say `untracked` for a call that merely inherits tracking
///   (docs/precision-backlog.md, 2026-09-17), so this is an upper bound on what
///   a rule could read, not a count of claims one may read today.
export function misuseClasses(document, reference) {
  const id = typeof reference === "string" ? reference : reference?.summary;
  const operations = document.summaries?.[id]?.call?.operations ?? [];
  const classes = new Set();
  for (const operation of operations) {
    const relation = operation.owner ?? {};
    if (relation.requires === "required" || relation.requiresCleanup === "required") {
      classes.add("owner");
    }
    if (operation.kind === "read" && (operation.inputs ?? []).some(input => input?.kind === "parameter")) {
      classes.add("argumentRead");
    }
    if (operation.kind === "return" && outputCarriesAccessor(operation.output)) {
      classes.add("returnedAccessor");
    }
    if (operation.kind === "invoke" && typeof operation.tracking === "string") classes.add("invoke");
  }
  return [...classes].sort();
}

/// Whether a wire shape may be invoked, exactly as `shape_may_be_callable`
/// (rust/crates/solid-reactive-ir/src/contracts.rs) decides it for the
/// normalized one: `callable`, `component` and `unknown` may be, a choice may be
/// unless its alternatives are closed and none of them may, and every other
/// shape is proven not to be. A summary with no shape fails closed.
function shapeMayBeCallable(shape) {
  if (shape === undefined || shape === null) return true;
  if (typeof shape === "string") return shape !== "plain";
  if (shape.kind !== "choice") return false;
  const closed = (shape.closed ?? []).includes("alternatives");
  return !closed || (shape.alternatives ?? []).some(shapeMayBeCallable);
}

/// What one summary leaves open to the consumer who imports it.
///
/// `value` is an export whose shape is proven not callable:
/// `project_export_semantics` closes every call-path domain of one, since it is
/// never invoked. `clean` is a callable with all four consumer domains closed.
/// `some-uses` leaves only `returns` or `callbacks` open, so whether SC9005
/// fires depends on how the import is used; `every-import` leaves `reads` or
/// `creates` open, so it fires wherever the name is imported. A reference the
/// document does not carry is `every-import`: nothing unstated closes a domain.
export function consumerState(document, reference) {
  const id = typeof reference === "string" ? reference : reference?.summary;
  const summary = document.summaries?.[id];
  if (!summary) return "every-import";
  if (!shapeMayBeCallable(summary.shape)) return "value";
  const closed = new Set(summary.call?.closed ?? []);
  const open = CONSUMER_DOMAINS.filter(domain => !closed.has(domain));
  if (open.length === 0) return "clean";
  return open.some(domain => OPEN_AT_EVERY_IMPORT.includes(domain)) ? "every-import" : "some-uses";
}

export function nameableEntrypoint(entrypoint) {
  return !WILDCARD_REACHED.test(entrypoint);
}

/// package name -> { exportName -> state }, plus the owner-requirement names and
/// each export's consumer view. The two views keep their best answers
/// independently, because each is the one a consumer gets for its own question.
export function surfacesFromDocuments(documents) {
  const surfaces = new Map();
  for (const document of documents) {
    const name = document.package?.name;
    if (!name) continue;
    const surface = surfaces.get(name) ?? {
      states: new Map(),
      owners: new Set(),
      consumer: new Map(),
      misuse: new Map()
    };
    surfaces.set(name, surface);
    for (const [entrypoint, value] of Object.entries(document.entrypoints ?? {})) {
      if (!nameableEntrypoint(entrypoint)) continue;
      for (const artifactCase of value.cases ?? []) {
        for (const [exportName, reference] of Object.entries(artifactCase.exports ?? {})) {
          const { state, owner } = summaryState(document, reference);
          if (owner) surface.owners.add(exportName);
          const held = surface.states.get(exportName);
          if (!held || RANK[state] > RANK[held]) surface.states.set(exportName, state);
          const classes = surface.misuse.get(exportName) ?? new Set();
          for (const kind of misuseClasses(document, reference)) classes.add(kind);
          surface.misuse.set(exportName, classes);
          const view = consumerState(document, reference);
          const heldView = surface.consumer.get(exportName);
          if (!heldView || CONSUMER_RANK[view] > CONSUMER_RANK[heldView]) {
            surface.consumer.set(exportName, view);
          }
        }
      }
    }
  }
  return surfaces;
}

export function census(demandRows, surfaces) {
  const byPackage = new Map();
  for (const row of demandRows) {
    const entry = byPackage.get(row.package) ?? [];
    entry.push(row);
    byPackage.set(row.package, entry);
  }
  const packages = [];
  const totals = { sites: 0, ownerRequirement: 0, measuredPackages: 0 };
  for (const state of STATES) totals[state] = 0;
  totals.unmeasured = 0;
  // Over in-surface sites only: an `absent` or unmeasured site has no summary
  // for an import to find anything open in, and is already its own bucket.
  totals.consumer = Object.fromEntries(CONSUMER_STATES.map(state => [state, 0]));

  for (const [name, rows] of [...byPackage].sort(
    (left, right) =>
      right[1].reduce((sum, row) => sum + row.sites, 0) -
        left[1].reduce((sum, row) => sum + row.sites, 0) || left[0].localeCompare(right[0])
  )) {
    const sites = rows.reduce((sum, row) => sum + row.sites, 0);
    totals.sites += sites;
    const surface = surfaces.get(name);
    if (!surface) {
      // No catalog published this package at all. Counted apart from `absent`,
      // which is a *measured* verdict about a package whose catalog is here.
      totals.unmeasured += sites;
      packages.push({ package: name, sites, measured: false });
      continue;
    }
    totals.measuredPackages += 1;
    const perState = Object.fromEntries(STATES.map(state => [state, 0]));
    const consumer = Object.fromEntries(CONSUMER_STATES.map(state => [state, 0]));
    let ownerRequirement = 0;
    for (const row of rows) {
      const state = surface.states.get(row.export) ?? "absent";
      perState[state] += row.sites;
      totals[state] += row.sites;
      if (surface.owners.has(row.export)) ownerRequirement += row.sites;
      if (state !== "absent") {
        // Both maps are filled from the same exports, so a miss here is a
        // defect in this script; failing closed keeps it from reading as a gain.
        const view = surface.consumer?.get(row.export) ?? "every-import";
        consumer[view] += row.sites;
        totals.consumer[view] += row.sites;
      }
    }
    // `totals[state]` was already accumulated per row above; adding
    // `perState` here counted every operation site twice.
    totals.ownerRequirement += ownerRequirement;
    packages.push({
      package: name,
      sites,
      measured: true,
      ...perState,
      ownerRequirement,
      consumer
    });
  }
  return { totals, packages, reported: reported(demandRows, surfaces) };
}

/// Numbers reported beside the gate and never compared (ways-to-improve
/// § 3.1). Neither depends on demand, so neither moves with the demand sweep:
///
/// - `surface`: per package, the share of its whole nameable export surface
///   whose import finds nothing open (a non-callable value, or a callable with
///   all four consumer domains closed), over every export its catalogs publish
///   at a nameable entrypoint -- demanded or not.
/// - `misuseCapable`: exports carrying a positive claim a misuse rule could
///   consume (`misuseClasses`), by class and in union, with the demanded sites
///   they cover under the frozen denominator.
export function reported(demandRows, surfaces) {
  const surface = [];
  const byClass = {};
  const capable = new Set();
  for (const [name, entry] of [...surfaces].sort((left, right) => left[0].localeCompare(right[0]))) {
    const views = [...(entry.consumer ?? new Map()).values()];
    const clean = views.filter(view => view === "clean").length;
    const value = views.filter(view => view === "value").length;
    surface.push({
      package: name,
      exports: views.length,
      value,
      clean,
      share: views.length === 0 ? 0 : Number(((clean + value) / views.length).toFixed(4))
    });
    for (const [exportName, classes] of entry.misuse ?? new Map()) {
      if (classes.size === 0) continue;
      capable.add(`${name}|${exportName}`);
      for (const kind of classes) byClass[kind] = (byClass[kind] ?? 0) + 1;
    }
  }
  let demandedSites = 0;
  const demandedExports = new Set();
  for (const row of demandRows) {
    const key = `${row.package}|${row.export}`;
    if (!capable.has(key)) continue;
    demandedSites += row.sites;
    demandedExports.add(key);
  }
  return {
    surface,
    misuseCapable: {
      exports: capable.size,
      byClass: Object.fromEntries(Object.entries(byClass).sort()),
      demandedExports: demandedExports.size,
      demandedSites
    }
  };
}

function share(part, whole) {
  return whole === 0 ? "0.0%" : `${((100 * part) / whole).toFixed(1)}%`;
}

function render({ totals, packages, reported: extra }) {
  const lines = [];
  lines.push(
    `${"package".padEnd(34)} ${"sites".padStart(6)} ${"ops".padStart(6)} ${"closed".padStart(7)} ${"degen".padStart(6)} ${"absent".padStart(7)} ${"owner".padStart(6)}`
  );
  for (const row of packages) {
    if (!row.measured) {
      lines.push(`${row.package.padEnd(34)} ${String(row.sites).padStart(6)}   (no catalog published this package)`);
      continue;
    }
    lines.push(
      `${row.package.padEnd(34)} ${String(row.sites).padStart(6)} ${String(row.operations).padStart(6)} ${String(row["closed-empty"]).padStart(7)} ${String(row.degenerate).padStart(6)} ${String(row.absent).padStart(7)} ${String(row.ownerRequirement).padStart(6)}`
    );
  }
  const measured = totals.sites - totals.unmeasured;
  lines.push("");
  lines.push(`measured call sites:               ${measured} of ${totals.sites}`);
  lines.push(
    `an operation is stated:            ${totals.operations} (${share(totals.operations, measured)})`
  );
  lines.push(
    `determined: states nothing:        ${totals["closed-empty"]} (${share(totals["closed-empty"], measured)})`
  );
  lines.push(
    `degenerate: nothing determined:    ${totals.degenerate} (${share(totals.degenerate, measured)})`
  );
  lines.push(
    `absent: not in the export surface: ${totals.absent} (${share(totals.absent, measured)})`
  );
  lines.push(
    `carries an owner requirement:      ${totals.ownerRequirement} (${share(totals.ownerRequirement, measured)})`
  );

  const inSurface = totals.operations + totals["closed-empty"] + totals.degenerate;
  const view = totals.consumer;
  lines.push("");
  lines.push(`what an import finds open, over the ${inSurface} in-surface sites:`);
  lines.push(`  nothing, a non-callable value:     ${view.value} (${share(view.value, inSurface)})`);
  lines.push(`  nothing, a callable:               ${view.clean} (${share(view.clean, inSurface)})`);
  lines.push(
    `  returns or callbacks: some uses:   ${view["some-uses"]} (${share(view["some-uses"], inSurface)})`
  );
  lines.push(
    `  reads or creates: every import:    ${view["every-import"]} (${share(view["every-import"], inSurface)})`
  );
  lines.push("");
  lines.push(
    `${"package".padEnd(34)} ${"value".padStart(6)} ${"clean".padStart(6)} ${"some".padStart(6)} ${"every".padStart(6)}`
  );
  for (const row of packages) {
    if (!row.measured) continue;
    const { consumer } = row;
    lines.push(
      `${row.package.padEnd(34)} ${String(consumer.value).padStart(6)} ${String(consumer.clean).padStart(6)} ${String(consumer["some-uses"]).padStart(6)} ${String(consumer["every-import"]).padStart(6)}`
    );
  }
  if (extra) {
    lines.push("");
    lines.push("reported, not gated -- whole nameable surface per package (value + clean over exports):");
    for (const row of extra.surface) {
      lines.push(
        `${row.package.padEnd(34)} ${String(row.value + row.clean).padStart(6)} of ${String(row.exports).padStart(4)}  ${share(row.value + row.clean, row.exports)}`
      );
    }
    const misuse = extra.misuseCapable;
    lines.push("");
    lines.push(
      `reported, not gated -- misuse-capable exports: ${misuse.exports} ${JSON.stringify(misuse.byClass)}; ` +
        `${misuse.demandedExports} demanded, ${misuse.demandedSites} demanded sites`
    );
  }
  return lines.join("\n");
}

/// What may not get worse. Stated as a direction per metric rather than as an
/// exact expectation: a new certified entrypoint *should* move these, and a
/// gate that refuses every movement is one nobody re-pins honestly.
const DIRECTIONS = [
  ["operations", "up", "sites with a stated operation"],
  ["ownerRequirement", "up", "sites carrying an owner requirement"],
  ["degenerate", "down", "sites where nothing was determined"],
  ["absent", "down", "sites whose export is not in the certified surface"],
  ["unmeasured", "down", "sites whose package published no catalog"]
];

/// The consumer view's directions, over its buckets rather than one per bucket.
/// `value` and `clean` are gated as their sum because a site trading one for the
/// other still finds nothing open. `some-uses` has no direction: it rises when an
/// `every-import` site improves and falls when one of its own becomes clean, so
/// either movement can be progress, and every regression into or out of it
/// already moves one of these two.
const CONSUMER_DIRECTIONS = [
  [view => (view.value ?? 0) + (view.clean ?? 0), "up", "sites whose import finds nothing open"],
  [
    view => view["every-import"] ?? 0,
    "down",
    "sites whose import raises SC9005 wherever the name is imported"
  ]
];

export function compare(pinned, actual) {
  const regressions = [];
  for (const [metric, direction, label] of DIRECTIONS) {
    const before = pinned.totals[metric] ?? 0;
    const now = actual.totals[metric] ?? 0;
    if (direction === "up" && now < before) {
      regressions.push(`${label}: ${before} -> ${now}`);
    }
    if (direction === "down" && now > before) {
      regressions.push(`${label}: ${before} -> ${now}`);
    }
  }
  // A pin written before the consumer view existed states nothing about it, and
  // reading that silence as zero would fail every run on `every-import`. The
  // view is compared from the first pin that carries it.
  const pinnedView = pinned.totals.consumer;
  const actualView = actual.totals.consumer;
  if (pinnedView && actualView) {
    for (const [read, direction, label] of CONSUMER_DIRECTIONS) {
      const before = read(pinnedView);
      const now = read(actualView);
      if ((direction === "up" && now < before) || (direction === "down" && now > before)) {
        regressions.push(`${label}: ${before} -> ${now}`);
      }
    }
  }
  return regressions;
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  const demand = JSON.parse(readFileSync(DEMAND, "utf8"));
  if (!Array.isArray(demand.rows) || demand.rows.length === 0) {
    fail("the pinned consumer demand has no rows");
  }
  if (options.printPackages) {
    // Derived, never restated. The set of packages to certify *is* the set the
    // pinned demand names; a hand-maintained list in the Makefile would drift
    // from it silently and shrink the denominator without anyone noticing.
    console.log([...new Set(demand.rows.map(row => row.package))].sort().join("\n"));
    return;
  }
  // Where the catalogs are. `--run` is the supported route: a `--keep-temp`
  // benchmark records each row's retained output directory in its report, so
  // nothing has to guess a path or -- the mistake this replaced -- force
  // `TMPDIR` inside the repository, where the probe harness correctly refuses
  // to run beside the repo's own `node_modules` ("probe write isolation was
  // violated") and ten of eighteen packages failed to certify at all.
  const roots = [];
  let report = null;
  if (options.run) {
    report = JSON.parse(readFileSync(resolve(options.run), "utf8"));
    for (const result of report.results ?? []) {
      const directory = result.retainedArtifacts?.outputDir;
      if (directory) roots.push(directory);
    }
    if (roots.length === 0) {
      fail(`${options.run} retained no output directories; the run needs --keep-temp`);
    }
  }
  if (options.catalogs) roots.push(resolve(options.catalogs));
  for (const directory of roots) {
    try {
      if (!statSync(directory).isDirectory()) fail(`${directory} is not a directory`);
    } catch {
      fail(`${directory} does not exist; run \`make contract-coverage-census\``);
    }
  }
  const documents = roots
    .flatMap(directory => publishedDocuments(directory))
    .map(path => JSON.parse(readFileSync(path, "utf8")));
  if (documents.length === 0) fail(`no published catalog documents under ${roots.join(", ")}`);
  const result = census(demand.rows, surfacesFromDocuments(documents));

  if (options.json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    console.log(render(result));
  }

  const record = {
    format: "solid-checker-contract-coverage-census",
    // 2 adds the consumer view to `totals` and to every measured package.
    censusVersion: 2,
    solidTarget: censusTarget(report),
    demand: {
      source: "docs/package-contract-v2/phase21/2026-09-14-consumer-demand-recensus.json",
      callSites: demand.totals?.callSites ?? null,
      inCorpusSites: demand.totals?.inCorpusSites ?? null
    },
    totals: result.totals,
    packages: result.packages,
    // Reported beside the gate; `compare` never reads it.
    reported: result.reported
  };
  if (options.update) {
    writeFileSync(PIN, `${JSON.stringify(record, null, 2)}\n`);
    console.log(`\npinned ${PIN}`);
    return;
  }
  let pinned;
  try {
    pinned = JSON.parse(readFileSync(PIN, "utf8"));
  } catch {
    fail(`no pin at ${PIN}; run with --update once the numbers are intentional`);
  }
  // ADR 0110 s 5: a coverage percentage measured after the Solid 1.x
  // retirement is not comparable to one measured before unless the denominator
  // is held. The pinned baseline was measured over the `solid1` corpus; a
  // `solid2` run measures a *narrower* one, which raises every percentage
  // without proving a single new statement. Refuse the comparison rather than
  // report a green run that compared two different questions.
  const pinnedTarget = pinned.solidTarget ?? "solid1";
  if (pinnedTarget !== record.solidTarget) {
    fail(
      `the pin was measured over the ${pinnedTarget} corpus and this run measured ${record.solidTarget}. ` +
        "Those denominators differ, so the comparison would be meaningless (ADR 0110 s 5). " +
        "Re-pin deliberately with --update, and record the new baseline beside the old one rather than as a continuation of it."
    );
  }
  const regressions = compare(pinned, result);
  if (regressions.length > 0) {
    console.error("\ncontract-coverage-census: coverage regressed");
    for (const regression of regressions) console.error(`  - ${regression}`);
    console.error("\nInspect, then re-pin with --update only if the movement is intended.");
    process.exit(1);
  }
  if (!pinned.totals?.consumer) {
    console.log(
      "\nthe pin predates the consumer view, so only the census buckets were compared; " +
        "re-pin with --update to gate it"
    );
  }
  console.log("\ncoverage did not regress against the pin");
}

if (import.meta.main) main();
