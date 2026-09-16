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
// # The one classification rule that is easy to get wrong
//
// An export counts only at an entrypoint a consumer **can name**.
// `policy2_artifact_acceptance_root` binds the entrypoint, so a summary
// published at `./src/dom.ts` -- reachable only because the package declares a
// `./*` wildcard -- does not answer `import { contains } from "@kobalte/utils"`.
// Crediting those inflated the 2026-09-15 census, and it is the difference
// between "this package states something" and "this package states something
// *to its consumers*".

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

function fail(message) {
  console.error(`contract-coverage-census: ${message}`);
  process.exit(1);
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

export function nameableEntrypoint(entrypoint) {
  return !WILDCARD_REACHED.test(entrypoint);
}

/// package name -> { exportName -> state }, plus the owner-requirement names.
export function surfacesFromDocuments(documents) {
  const surfaces = new Map();
  for (const document of documents) {
    const name = document.package?.name;
    if (!name) continue;
    const surface = surfaces.get(name) ?? { states: new Map(), owners: new Set() };
    surfaces.set(name, surface);
    for (const [entrypoint, value] of Object.entries(document.entrypoints ?? {})) {
      if (!nameableEntrypoint(entrypoint)) continue;
      for (const artifactCase of value.cases ?? []) {
        for (const [exportName, reference] of Object.entries(artifactCase.exports ?? {})) {
          const { state, owner } = summaryState(document, reference);
          if (owner) surface.owners.add(exportName);
          const held = surface.states.get(exportName);
          if (!held || RANK[state] > RANK[held]) surface.states.set(exportName, state);
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
    let ownerRequirement = 0;
    for (const row of rows) {
      const state = surface.states.get(row.export) ?? "absent";
      perState[state] += row.sites;
      totals[state] += row.sites;
      if (surface.owners.has(row.export)) ownerRequirement += row.sites;
    }
    // `totals[state]` was already accumulated per row above; adding
    // `perState` here counted every operation site twice.
    totals.ownerRequirement += ownerRequirement;
    packages.push({
      package: name,
      sites,
      measured: true,
      ...perState,
      ownerRequirement
    });
  }
  return { totals, packages };
}

function share(part, whole) {
  return whole === 0 ? "0.0%" : `${((100 * part) / whole).toFixed(1)}%`;
}

function render({ totals, packages }) {
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
    `absent: the entrypoint refused:    ${totals.absent} (${share(totals.absent, measured)})`
  );
  lines.push(
    `carries an owner requirement:      ${totals.ownerRequirement} (${share(totals.ownerRequirement, measured)})`
  );
  return lines.join("\n");
}

/// What may not get worse. Stated as a direction per metric rather than as an
/// exact expectation: a new certified entrypoint *should* move these, and a
/// gate that refuses every movement is one nobody re-pins honestly.
const DIRECTIONS = [
  ["operations", "up", "sites with a stated operation"],
  ["ownerRequirement", "up", "sites carrying an owner requirement"],
  ["degenerate", "down", "sites where nothing was determined"],
  ["absent", "down", "sites whose entrypoint refused"],
  ["unmeasured", "down", "sites whose package published no catalog"]
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
  if (options.run) {
    const report = JSON.parse(readFileSync(resolve(options.run), "utf8"));
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
    censusVersion: 1,
    demand: {
      source: "docs/package-contract-v2/phase21/2026-09-14-consumer-demand-recensus.json",
      callSites: demand.totals?.callSites ?? null,
      inCorpusSites: demand.totals?.inCorpusSites ?? null
    },
    totals: result.totals,
    packages: result.packages
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
  const regressions = compare(pinned, result);
  if (regressions.length > 0) {
    console.error("\ncontract-coverage-census: coverage regressed");
    for (const regression of regressions) console.error(`  - ${regression}`);
    console.error("\nInspect, then re-pin with --update only if the movement is intended.");
    process.exit(1);
  }
  console.log("\ncoverage did not regress against the pin");
}

if (import.meta.main) main();
