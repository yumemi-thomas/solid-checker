#!/usr/bin/env bun

// Does the checked-in probe-recipe corpus still address anything?
//
//   bun scripts/probe-recipe-addressing.mjs --run <run.json>
//   bun scripts/probe-recipe-addressing.mjs --run <run.json> --json
//   bun scripts/probe-recipe-addressing.mjs --run <run.json> --update
//
// A recipe is addressed by `claimId`, a content digest over the exact
// normalized claim -- package, version, artifact case, export, domain. The
// corpus README states the consequence and nothing enforced it: *anything that
// moves the emitted contract document moves the id, and the recipe then
// addresses nothing.* The failure is silent in both directions. Rust withholds
// the candidate as `no recipe in corpus` and the row still certifies; the
// corpus keeps a module nobody runs, and a later regeneration adds a second
// family beside it rather than replacing it.
//
// Measured 2026-09-16 against the pinned census run: **1 of 325 recipes
// addressed a claim the run proposed.** All 159 `solid-primitives-utils-*`
// modules addressed none -- including the twenty the corpus README describes as
// closing `@solid-primitives/utils@6.4.1`'s `.` reads domains, against a run
// that certified exactly that package at exactly that version. The corpus holds
// five separate `noop`-reads recipes, one per historical artifact case, and all
// five are dead.
//
// # What counts as stale, and what does not
//
// A recipe for a package the run never certified is *out of scope*, not stale:
// the corpus covers rows this census does not certify (`seroval`, `motion`,
// `corvu`, `@tanstack/store`), and calling those dead would make the number
// depend on which packages a run happened to include. So scope is decided by
// the module's own bare imports -- the package a recipe imports is the package
// it is about -- and only an in-scope recipe that addresses nothing is stale.
//
// # The cost side
//
// A withheld `no recipe in corpus` candidate leaves its claim domain open, so
// the export's summary stays degenerate and its consumers learn nothing. This
// prices that in the same units as `contract-coverage-census.mjs`: consumer
// call sites from the pinned demand sweep, counted **only at entrypoints a
// consumer can name**, because `policy2_artifact_acceptance_root` binds the
// entrypoint and a claim at a `./*`-reached source file answers no import
// anybody writes.

import { readFileSync, writeFileSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

import { nameableEntrypoint } from "./contract-coverage-census.mjs";

const root = resolve(import.meta.dirname, "..");
const CORPUS = join(root, "scripts/ecosystem-benchmark/probe-recipes");
const DEMAND = join(
  root,
  "docs/package-contract-v2/phase21/2026-09-14-consumer-demand-recensus.json"
);
const PIN = join(root, "benchmarks/ecosystem/probe-recipe-addressing.json");

/// Specifiers a recipe imports for the apparatus rather than for the subject.
const HARNESS_SPECIFIERS = new Set(["node:assert", "node:test"]);

function fail(message) {
  console.error(`probe-recipe-addressing: ${message}`);
  process.exit(1);
}

function parseArguments(argv) {
  const options = { run: null, catalogs: null, json: false, update: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--run") options.run = argv[++index];
    else if (argument === "--catalogs") options.catalogs = argv[++index];
    else if (argument === "--json") options.json = true;
    else if (argument === "--update") options.update = true;
    else if (argument === "-h" || argument === "--help") {
      console.log(readFileSync(new URL(import.meta.url), "utf8").split("\n\n")[0]);
      process.exit(0);
    } else fail(`unknown argument ${JSON.stringify(argument)}`);
  }
  if (!options.run && !options.catalogs) {
    fail("one of --run <run.json> or --catalogs <DIR> is required");
  }
  return options;
}

function walk(directory, suffix, found = []) {
  let entries;
  try {
    entries = readdirSync(directory, { withFileTypes: true });
  } catch {
    return found;
  }
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) walk(path, suffix, found);
    else if (entry.name.endsWith(suffix)) found.push(path);
  }
  return found;
}

/// The package a recipe is about, read off the module's own bare imports.
///
/// Deliberately not the filename and not `coverageLimitations`. The filename is
/// a convention three families already break (`kobalte-utils-092-browser-*`,
/// `web-with-meta`), and the limitation prose names a version in four different
/// shapes. What a recipe imports is what it probes, and Rust copies these exact
/// bytes, so this is the one attribution that cannot drift from the artifact.
export function recipeSubjects(source) {
  const subjects = new Set();
  const specifiers = [
    ...source.matchAll(/(?:^|\s)(?:import|export)[^;]*?from\s*["']([^"']+)["']/g),
    ...source.matchAll(/\bimport\s*\(\s*["']([^"']+)["']\s*\)/g),
    ...source.matchAll(/(?:^|\s)import\s+["']([^"']+)["']/g)
  ].map(match => match[1]);
  for (const specifier of specifiers) {
    if (specifier.startsWith(".") || specifier.startsWith("/")) continue;
    if (HARNESS_SPECIFIERS.has(specifier)) continue;
    if (specifier.startsWith("node:")) continue;
    const parts = specifier.split("/");
    subjects.add(specifier.startsWith("@") ? parts.slice(0, 2).join("/") : parts[0]);
  }
  return subjects;
}

/// Every claim a run *proposed*, and every claim it withheld for want of a
/// recipe. Proposals are the right denominator: they list each candidate
/// whether or not a recipe served it, so "addressed" means the id still names
/// something rather than "the recipe happened to be needed".
export function runClaims(roots) {
  const proposed = new Map();
  const withheld = [];
  for (const directory of roots) {
    for (const path of walk(directory, ".proposal.json")) {
      let document;
      try {
        document = JSON.parse(readFileSync(path, "utf8"));
      } catch {
        continue;
      }
      for (const candidate of document.closureCandidates ?? []) {
        if (!candidate.claimId) continue;
        proposed.set(candidate.claimId, {
          entrypoint: candidate.artifact?.entrypoint ?? "",
          export: candidate.subject?.export ?? "",
          domain: candidate.subject?.path?.domain ?? ""
        });
      }
    }
    for (const path of walk(directory, ".certification-audit.json")) {
      let document;
      try {
        document = JSON.parse(readFileSync(path, "utf8"));
      } catch {
        continue;
      }
      const name = document.package?.name ?? "";
      for (const record of document.withheldClosures ?? []) {
        if (record.reason !== "no recipe in corpus") continue;
        withheld.push({
          package: name,
          export: record.export ?? "",
          domain: record.domain ?? "",
          claimId: record.semanticClaimId ?? ""
        });
      }
    }
  }
  return { proposed, withheld };
}

export function addressing({ recipes, proposed, withheld, certified, sites }) {
  const families = new Map();
  for (const recipe of recipes) {
    const inScope = [...recipe.subjects].some(name => certified.has(name));
    const addressed = proposed.has(recipe.claimId);
    const key = [...recipe.subjects].sort().join(",") || "(no bare import)";
    const family = families.get(key) ?? {
      subject: key,
      recipes: 0,
      addressed: 0,
      inScope,
      stale: 0
    };
    family.recipes += 1;
    family.inScope = family.inScope || inScope;
    if (addressed) family.addressed += 1;
    else if (inScope) family.stale += 1;
    families.set(key, family);
  }

  // The price of every unserved candidate, in the census's own units: a claim
  // withheld for want of a recipe leaves its domain open, and only a claim at a
  // nameable entrypoint reaches a consumer at all.
  //
  // Fails closed on the entrypoint, which is not a formality here. A run's
  // dependency-graph nodes certify without retaining a proposal, so most
  // withheld records cannot be placed at an entrypoint from these artifacts --
  // and `@kobalte/utils` is certified almost entirely at `./src/*.ts` cases a
  // `./*` wildcard reaches, which answer no import a consumer writes. Counting
  // the unplaceable ones priced `mergeDefaultProps` at 254 sites it does not
  // have: the same inflation `contract-coverage-census.mjs`'s header records.
  // They are reported apart instead, as the measurement this cannot make.
  const unserved = new Map();
  let unplaceable = 0;
  for (const record of withheld) {
    const claim = proposed.get(record.claimId);
    if (!claim) {
      unplaceable += 1;
      continue;
    }
    if (!nameableEntrypoint(claim.entrypoint)) continue;
    const key = `${record.package}::${record.export}`;
    const held = unserved.get(key) ?? { key, sites: sites.get(key) ?? 0, domains: new Set() };
    held.domains.add(record.domain);
    unserved.set(key, held);
  }

  const ordered = [...families.values()].sort(
    (left, right) => right.recipes - left.recipes || left.subject.localeCompare(right.subject)
  );
  const totals = {
    recipes: recipes.length,
    addressed: ordered.reduce((sum, family) => sum + family.addressed, 0),
    stale: ordered.reduce((sum, family) => sum + family.stale, 0),
    outOfScope: ordered
      .filter(family => !family.inScope)
      .reduce((sum, family) => sum + family.recipes - family.addressed, 0),
    unservedClaims: withheld.length,
    unplaceableClaims: unplaceable,
    unservedExports: unserved.size,
    unservedSites: [...unserved.values()].reduce((sum, entry) => sum + entry.sites, 0)
  };
  return {
    totals,
    families: ordered.map(({ subject, recipes: count, addressed, stale, inScope }) => ({
      subject,
      recipes: count,
      addressed,
      stale,
      inScope
    })),
    unserved: [...unserved.values()]
      .sort((left, right) => right.sites - left.sites || left.key.localeCompare(right.key))
      .map(entry => ({
        export: entry.key,
        sites: entry.sites,
        domains: [...entry.domains].sort()
      }))
  };
}

function render({ totals, families, unserved }) {
  const lines = [];
  lines.push(
    `${"recipe subject".padEnd(40)} ${"recipes".padStart(7)} ${"addressed".padStart(9)} ${"stale".padStart(6)}`
  );
  for (const family of families) {
    lines.push(
      `${family.subject.padEnd(40)} ${String(family.recipes).padStart(7)} ${String(family.addressed).padStart(9)} ${
        family.inScope ? String(family.stale).padStart(6) : "     -"
      }`
    );
  }
  lines.push("");
  lines.push(`recipes in the corpus:                   ${totals.recipes}`);
  lines.push(`addressing a claim this run proposed:    ${totals.addressed}`);
  lines.push(`stale (this run certified the package):  ${totals.stale}`);
  lines.push(`out of scope (package not in this run):  ${totals.outOfScope}`);
  lines.push("");
  lines.push(`claims withheld for want of a recipe:    ${totals.unservedClaims}`);
  lines.push(`  of those, placed at a nameable case:   ${totals.unservedClaims - totals.unplaceableClaims}`);
  lines.push(`  of those, no retained proposal places: ${totals.unplaceableClaims}`);
  lines.push(`demanded exports left open by that:      ${totals.unservedExports}`);
  lines.push(`consumer call sites they cost:           ${totals.unservedSites}`);
  if (unserved.length > 0) {
    lines.push("");
    lines.push("most expensive exports left open:");
    for (const entry of unserved.slice(0, 15)) {
      if (entry.sites === 0) continue;
      lines.push(
        `  ${String(entry.sites).padStart(4)}  ${entry.export.padEnd(46)} ${entry.domains.join("/")}`
      );
    }
  }
  return lines.join("\n");
}

/// Movement that must not happen silently. A corpus that addresses fewer claims
/// than the pin, or leaves more consumer sites open, has gone stale since it was
/// last reviewed -- which is the whole failure this exists to make loud.
const DIRECTIONS = [
  ["addressed", "up", "recipes addressing a claim"],
  ["stale", "down", "stale recipes"],
  ["unservedSites", "down", "consumer sites left open"]
];

export function compare(pinned, actual) {
  const regressions = [];
  for (const [metric, direction, label] of DIRECTIONS) {
    const before = pinned.totals?.[metric] ?? 0;
    const now = actual.totals[metric] ?? 0;
    if (direction === "up" && now < before) regressions.push(`${label}: ${before} -> ${now}`);
    if (direction === "down" && now > before) regressions.push(`${label}: ${before} -> ${now}`);
  }
  return regressions;
}

function main() {
  const options = parseArguments(process.argv.slice(2));

  const roots = [];
  const certified = new Set();
  if (options.run) {
    const report = JSON.parse(readFileSync(resolve(options.run), "utf8"));
    for (const result of report.results ?? []) {
      if (result.package) certified.add(result.package);
      if (result.retainedArtifacts?.outputDir) roots.push(result.retainedArtifacts.outputDir);
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

  const manifest = JSON.parse(readFileSync(join(CORPUS, "recipes.json"), "utf8"));
  const recipes = (manifest.recipes ?? []).map(entry => ({
    module: entry.module,
    claimId: entry.claimId,
    subjects: recipeSubjects(readFileSync(join(CORPUS, entry.module), "utf8"))
  }));
  if (recipes.length === 0) fail(`${join(CORPUS, "recipes.json")} declares no recipes`);

  const demand = JSON.parse(readFileSync(DEMAND, "utf8"));
  const sites = new Map();
  for (const row of demand.rows ?? []) {
    const key = `${row.package}::${row.export}`;
    sites.set(key, (sites.get(key) ?? 0) + row.sites);
  }

  const { proposed, withheld } = runClaims(roots);
  if (proposed.size === 0) fail(`no certification proposals under ${roots.join(", ")}`);
  const result = addressing({ recipes, proposed, withheld, certified, sites });

  if (options.json) console.log(JSON.stringify(result, null, 2));
  else console.log(render(result));

  const record = {
    format: "solid-checker-probe-recipe-addressing",
    addressingVersion: 1,
    corpus: "scripts/ecosystem-benchmark/probe-recipes",
    demand: "docs/package-contract-v2/phase21/2026-09-14-consumer-demand-recensus.json",
    totals: result.totals,
    families: result.families,
    unserved: result.unserved
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
    console.error("\nprobe-recipe-addressing: the corpus addresses less than it did");
    for (const regression of regressions) console.error(`  - ${regression}`);
    console.error(
      "\nA recipe stops addressing anything when the contract document moves."
    );
    console.error(
      "Re-address it with scripts/probe-recipe-scaffold.mjs, then re-pin with --update."
    );
    process.exit(1);
  }
  console.log("\naddressing did not regress against the pin");
}

if (import.meta.main) main();
