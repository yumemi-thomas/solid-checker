#!/usr/bin/env bun
// Regenerates the compiled-in accepted-contract tier from a certification run.
//
//   bun scripts/bundle-accepted-contracts.mjs --run <run.json> [--run <run.json> ...]
//   bun scripts/bundle-accepted-contracts.mjs --catalogs <DIR> --dry-run
//
// The inputs are the published catalogs `--keep-temp` ecosystem runs left
// behind. The census run is one of them -- the same artifacts
// `contract-coverage-census.mjs` reads, so what the census measures is
// delivered -- and each consumer-environment run is another: a delivery-only
// certification in a tree a real consumer installs, which the census refuses
// and which is measured by re-sweeping that consumer instead (owner decision,
// 2026-09-26; see the Makefile's `accepted-bundles`). `--run` is repeatable
// for that reason. Their bundles cannot collide with the census's: a bundle is
// keyed by the dependency environment it was proven in (`bundleKey`). For each
// catalog this shells out to `solid-contract-bundle`, which authenticates the
// certification's own receipt and re-issues it as a built-in one. Nothing is
// re-proven here and this script proves nothing; see `contract_bundling.rs`.
//
// What lands: `pkg/contracts/accepted/index.json`, the content-addressed
// objects beside it, and the generated `include_bytes!` list the checker
// compiles in. All three are checked in, because a bundle's authority *is*
// that its bytes are reviewed in this repository.

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync
} from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { nameableEntrypoint } from "./contract-coverage-census.mjs";
import { loadConsumerEnvironments } from "./ecosystem-benchmark/lib/consumer-environments.mjs";

const REPOSITORY = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const BUNDLE_ROOT = join(REPOSITORY, "pkg/contracts/accepted");
const OBJECT_ROOT = join(BUNDLE_ROOT, "objects");
const INDEX_PATH = join(BUNDLE_ROOT, "index.json");
const EMBEDDED_PATH = join(
  REPOSITORY,
  "rust/crates/solid-facts-backend/src/accepted_bundles/embedded.rs"
);
const BUNDLER =
  process.env.SOLID_CONTRACT_BUNDLE_BIN
  ?? join(REPOSITORY, "rust/target/debug/solid-contract-bundle");

function fail(message) {
  console.error(`bundle-accepted-contracts: ${message}`);
  process.exit(1);
}

/** Throws on a malformed command line; `main` turns that into an exit. */
export function parseArguments(argv) {
  const options = { runs: [], catalogs: "", dryRun: false, allEntrypoints: false };
  const value = (index, message) => {
    const next = argv[index];
    if (next === undefined || next.startsWith("--")) throw new Error(message);
    return next;
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--run") {
      options.runs.push(value(++index, "--run needs a path"));
    } else if (argument === "--catalogs") {
      options.catalogs = value(++index, "--catalogs needs a directory");
    } else if (argument === "--dry-run") {
      options.dryRun = true;
    } else if (argument === "--all-entrypoints") {
      options.allEntrypoints = true;
    } else {
      throw new Error(`unknown argument ${argument}`);
    }
  }
  const resolved = options.runs.map(run => resolve(run));
  const repeated = resolved.find((run, index) => resolved.indexOf(run) !== index);
  if (repeated) throw new Error(`--run ${repeated} is given twice`);
  if (options.runs.length === 0 && !options.catalogs) {
    throw new Error("one of --run <run.json> or --catalogs <DIR> is required");
  }
  return options;
}

/**
 * The retained output directory of every result of every run, in the order
 * given. Throws for a run that retained nothing -- it was not a `--keep-temp`
 * run, and bundling from it would silently deliver less than was certified --
 * and for a consumer-environment run whose environment is not a reviewed one:
 * a delivery run is admitted into the tier only for an environment
 * scripts/ecosystem-benchmark/consumer-environments.json lists.
 */
export function retainedOutputDirectories(runPaths, { reviewedEnvironments = null } = {}) {
  const roots = [];
  for (const runPath of runPaths) {
    const report = JSON.parse(readFileSync(resolve(runPath), "utf8"));
    const environment = report?.scope?.consumerEnvironment;
    if (environment) {
      const reviewed = reviewedEnvironments ?? loadConsumerEnvironments().environments.map(entry => entry.id);
      if (!reviewed.includes(environment)) {
        throw new Error(
          `${runPath} is a delivery run for consumer environment ${environment}, ` +
            "which scripts/ecosystem-benchmark/consumer-environments.json does not list"
        );
      }
    }
    const before = roots.length;
    for (const result of report.results ?? []) {
      const directory = result.retainedArtifacts?.outputDir;
      if (directory) roots.push(directory);
    }
    if (roots.length === before) {
      throw new Error(`${runPath} retained no output directories; the run needs --keep-temp`);
    }
  }
  return roots;
}

/** Every published `accepted-contracts.json` under a retained output tree. */
function publishedCatalogs(directory) {
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
      else if (entry.name === "accepted-contracts.json") found.push(path);
    }
  };
  walk(directory);
  return found.sort();
}

/**
 * The trust configuration the certification wrote for this catalog.
 *
 * The ecosystem runner writes it to `<catalogPath>.authority/trust.json`, where
 * `<catalogPath>` is the catalog *root* it passed -- so for a case set or a
 * graph the authority sits beside an ancestor of this file, not beside the file
 * itself. Walk up until one is found; without it the entry authenticates
 * against nothing and bundling refuses, which is the correct refusal.
 */
function trustConfigurationFor(catalog) {
  let current = dirname(catalog);
  for (let depth = 0; depth < 12; depth += 1) {
    const authority = `${current}.authority/trust.json`;
    if (existsSync(authority)) return authority;
    const parent = dirname(current);
    if (parent === current) break;
    current = parent;
  }
  return "";
}

function sha256(text) {
  return `sha256:${createHash("sha256").update(text).digest("hex")}`;
}

function bundleCatalog(catalog, trust) {
  const stdout = execFileSync(
    BUNDLER,
    ["--catalog", catalog, "--trust-configuration", trust],
    { encoding: "utf8", maxBuffer: 256 * 1024 * 1024 }
  );
  return JSON.parse(stdout);
}

/**
 * The dependency environment a bundle was proven in, canonically: every
 * package besides its own that the certification read, as the receipt's
 * `dependencyEnvironmentRoot` binds it. The Rust tool already refused an entry
 * whose published list does not reproduce that root, and one that states none.
 */
export function environmentOf(entry) {
  const environment = entry.dependencyEnvironment;
  if (!Array.isArray(environment)) {
    throw new Error(
      `${entry.packageName}@${entry.packageVersion} ${entry.requestedEntrypoint} states no dependency environment`
    );
  }
  return environment.map(environmentEntryKey).join(",");
}

/**
 * One environment entry, spelled out: its package identity and, when the
 * environment states its resolution edges (`resolvedFrom`, ADR 0123's
 * `edges:v3` root), who resolved it by which name. The edges are part of what
 * the root signs and of what admission replays, so two certifications that
 * read the same packages through different lookups are two bundles, never
 * one; and an edge-bearing environment never shares a key with the same
 * packages stated without edges, which admission reads by a different rule.
 */
function environmentEntryKey({ name, version, integrity, resolvedFrom }) {
  const identity = `${name}@${version}#${integrity}`;
  if (!resolvedFrom) return identity;
  const importer = resolvedFrom.importer === "certified"
    ? "certified"
    : `${resolvedFrom.importer.package.name}@${resolvedFrom.importer.package.version}#${
      resolvedFrom.importer.package.integrity}`;
  return `${identity}<-${resolvedFrom.specifier}@${importer}`;
}

/**
 * The one key a bundle is unique under: an artifact identity, spelled out, and
 * the environment it was proven in.
 *
 * The environment is part of the key because it is part of what was proven.
 * The ecosystem corpus certifies the same package bytes on a floor row
 * (`@solidjs/signals@2.0.0-rc.0`) and a head row (rc.6), and the head rows
 * close claims -- `@solid-primitives/utils` `createMicrotask` `creates` among
 * them -- that only rc.6's audited rows discharge. Keyed by artifact alone,
 * the "keep the closing certification" rule below shipped those closures to
 * rc.0 projects; keyed by environment, the floor and head certifications are
 * two bundles, and each is admitted only where its own environment is
 * installed.
 */
export function bundleKey(entry) {
  return [
    entry.packageName,
    entry.packageVersion,
    entry.requestedEntrypoint,
    entry.exportConditions.join("+"),
    environmentOf(entry)
  ].join(" | ");
}

/**
 * What a bundle's document *says*, canonically: every export of its one
 * artifact case, paired with the summary it resolves to.
 *
 * This is the comparator, and the document digest is not. One published
 * artifact is routinely certified more than once in a corpus run -- as a root
 * row and again as another package's dependency node -- and those documents
 * differ in bytes every time, because the artifact-case id, the closure digests
 * and the provenance all differ. Measured: two `@solid-primitives/keyed@1.5.3`
 * documents from one run, different digests, byte-identical summaries for all
 * six exports. Comparing digests called that a conflict; comparing claims calls
 * it what it is.
 *
 * Whole-document is exactly the one case's surface: an embedded bundle must
 * carry a single artifact case, which is what the per-case catalogs publish.
 */
export function claimsOf(documentText) {
  const document = JSON.parse(documentText);
  const bodies = new Map();
  const closed = new Map();
  for (const [entrypoint, value] of Object.entries(document.entrypoints ?? {})) {
    for (const artifactCase of value.cases ?? [value]) {
      for (const [name, reference] of Object.entries(artifactCase?.exports ?? {})) {
        const summary = document.summaries?.[reference] ?? null;
        const key = `${entrypoint}\u0000${name}`;
        bodies.set(key, canonicalBody(summary));
        closed.set(key, new Set(summary?.call?.closed ?? []));
      }
    }
  }
  return { bodies, closed };
}

/// The domains a `call` summary may close. An empty array for one of these is
/// meaningless on its own -- "states nothing here" and "proved there is nothing
/// here" are the same bytes -- and `closed` is what tells them apart, so the
/// body drops the empty arrays and the closure comparison carries the meaning.
const CLOSABLE = ["reads", "creates", "returns", "callbacks"];

export function canonicalBody(summary) {
  if (!summary) return "null";
  const call = { ...(summary.call ?? {}) };
  delete call.closed;
  delete call.proposedClosures;
  for (const domain of CLOSABLE) {
    if (Array.isArray(call[domain]) && call[domain].length === 0) delete call[domain];
  }
  return stableJson({ ...summary, call });
}

function stableJson(value) {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map(key => `${JSON.stringify(key)}:${stableJson(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value) ?? "null";
}

/**
 * How two certifications of one published artifact **in one environment**
 * relate. Only ever asked within one `bundleKey`: an open-versus-closed
 * difference between two environments is not a difference of demand scope,
 * and neither refines the other.
 *
 * Measured over a full census run: 89 artifact/entrypoint pairs, 87 where every
 * certification agreed exactly, 2 that differed, and **no contradiction at all**.
 * Both differences were one certification closing a claim domain the other left
 * open, with every other byte of every summary identical. The cause is
 * structural rather than incidental: a root row certifies the artifact against
 * its own full demand plan, while the same artifact reached as another
 * package's dependency node is asked only what that dependent needed.
 *
 * So an open domain is not a denial -- this repository's own rule, "an open leaf
 * is not negative proof" -- and keeping the certification that closed more
 * contradicts nothing the other one issued. Both are receipt-issued and
 * authenticated; neither is being merged, and no claim is invented. Anything
 * that is not this shape is still dropped, including two documents that close
 * *different* domains, because picking between those would be a guess.
 */
export function relate(left, right) {
  const keys = new Set([...left.bodies.keys(), ...right.bodies.keys()]);
  let leftCloses = false;
  let rightCloses = false;
  for (const key of keys) {
    if (!left.bodies.has(key) || !right.bodies.has(key)) return "conflict";
    if (left.bodies.get(key) !== right.bodies.get(key)) return "conflict";
    const leftClosed = left.closed.get(key);
    const rightClosed = right.closed.get(key);
    for (const domain of leftClosed) if (!rightClosed.has(domain)) leftCloses = true;
    for (const domain of rightClosed) if (!leftClosed.has(domain)) rightCloses = true;
  }
  if (leftCloses && rightCloses) return "conflict";
  if (leftCloses) return "refines";
  if (rightCloses) return "coarsens";
  return "same";
}

function main() {
  let options;
  let roots;
  try {
    options = parseArguments(process.argv.slice(2));
    roots = retainedOutputDirectories(options.runs);
  } catch (error) {
    fail(error.message);
  }
  if (!existsSync(BUNDLER)) {
    fail(`${BUNDLER} does not exist; run \`make build-checker-debug\``);
  }
  if (options.catalogs) roots.push(resolve(options.catalogs));
  for (const directory of roots) {
    try {
      if (!statSync(directory).isDirectory()) fail(`${directory} is not a directory`);
    } catch {
      fail(`${directory} does not exist`);
    }
  }

  const results = [];
  const refused = [];
  for (const catalog of roots.flatMap(publishedCatalogs)) {
    const trust = trustConfigurationFor(catalog);
    if (!trust) {
      refused.push([catalog, "no trust configuration beside or above the catalog"]);
      continue;
    }
    try {
      results.push(bundleCatalog(catalog, trust));
    } catch (error) {
      refused.push([catalog, String(error.stderr ?? error.message ?? "").trim()]);
    }
  }
  const { ordered, objects, conflicted, refinements } = collectBundles(results, options);
  const index = {
    format: "solid-checker-accepted-contract-bundle-index",
    bundleIndexVersion: BUNDLE_INDEX_VERSION,
    bundles: ordered
  };

  for (const [catalog, reason] of refused) {
    console.error(`  refused ${relative(REPOSITORY, catalog)}: ${reason.split("\n")[0]}`);
  }
  for (const key of [...conflicted].sort()) {
    console.error(`  dropped ${key}: two certifications of it do not agree`);
  }
  for (const key of [...refinements.keys()].sort()) {
    console.log(`  kept the closing certification of ${key}`);
  }
  const packages = new Set(ordered.map(entry => entry.packageName));
  console.log(`${ordered.length} bundle(s) over ${packages.size} package(s)`);
  for (const entry of ordered) {
    console.log(
      `  ${entry.packageName}@${entry.packageVersion} ${entry.requestedEntrypoint}`
      + ` (${entry.dependencyEnvironment.length} environment package(s))`
    );
  }
  if (options.dryRun) return;

  rmSync(OBJECT_ROOT, { recursive: true, force: true });
  mkdirSync(OBJECT_ROOT, { recursive: true });
  // Exactly what the index names. A run reaches many certifications of one
  // artifact and keeps one; the rest are not this build's business.
  const named = new Set(ordered.flatMap(entry => [entry.document, entry.receipt]));
  const members = [...objects.keys()].filter(member => named.has(member)).sort();
  for (const member of members) {
    const bytes = objects.get(member);
    const address = member.split("/").pop().split(".")[0];
    if (sha256(bytes) !== `sha256:${address}`) fail(`${member} is not at its content address`);
    writeFileSync(join(BUNDLE_ROOT, member), bytes);
  }
  writeFileSync(INDEX_PATH, `${JSON.stringify(index, null, 2)}\n`);
  writeFileSync(EMBEDDED_PATH, embeddedSource(members));
  console.log(`wrote ${relative(REPOSITORY, INDEX_PATH)} and ${members.length} object(s)`);
}

/// Version 2 states each bundle's dependency environment and keys bundles by
/// it. The loader still reads version 1, as inert: none of its bundles states
/// an environment, so none is ever admitted.
export const BUNDLE_INDEX_VERSION = 2;

/**
 * Every bundle a set of `solid-contract-bundle` results yields, one per
 * `bundleKey`, with the objects they name.
 *
 * Pure, so the floor/head behaviour is testable without a certification run:
 * `results` is exactly what the Rust tool prints, `{ bundles, objects }` per
 * catalog.
 */
export function collectBundles(results, options = {}) {
  const bundles = new Map();
  const objects = new Map();
  const conflicted = new Set();
  const refinements = new Map();
  for (const result of results) {
    for (const entry of result.bundles) {
      // An entrypoint reached only through a `./*` wildcard answers no import a
      // consumer writes, and the census does not count it either. Bundling one
      // would carry bytes nobody can reach. `--all-entrypoints` keeps them.
      if (!options.allEntrypoints && !nameableEntrypoint(entry.requestedEntrypoint)) continue;
      const key = bundleKey(entry);
      // A key that already contradicted itself stays dropped. A later
      // certification cannot resolve which of two contradicting ones describes
      // the bytes, and letting one overwrite the pair would hide the question.
      if (conflicted.has(key)) continue;
      const claims = claimsOf(result.objects[entry.document]);
      const previous = bundles.get(key);
      const relation = previous ? relate(claims, previous.claims) : "same";
      if (relation === "conflict") {
        // Two certifications of one published artifact that do not agree.
        // Neither may be applied -- which one describes the bytes is exactly
        // the question this cannot answer -- so the artifact is dropped and
        // named. Dropping one key must not stop the other bundles: a corpus
        // run reaches many packages, and one disagreement used to abort the
        // whole generation.
        conflicted.add(key);
        bundles.delete(key);
        continue;
      }
      // Keep the certification that closed more, and record that it happened:
      // a refinement is the difference between shipping a contract and shipping
      // nothing for that artifact, and it should be visible rather than quiet.
      if (relation === "refines") {
        refinements.set(key, [...(refinements.get(key) ?? []), entry.requestedEntrypoint]);
        bundles.set(key, { ...entry, claims });
      } else if (relation === "same") {
        // Deterministic among documents that agree, so regenerating the same run
        // reproduces the same bytes whatever order the catalogs were walked in.
        if (!previous || entry.documentDigest < previous.documentDigest) {
          bundles.set(key, { ...entry, claims });
        }
      }
      for (const member of [entry.document, entry.receipt]) {
        objects.set(member, result.objects[member]);
      }
    }
  }

  for (const key of conflicted) bundles.delete(key);
  const ordered = [...bundles.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([, entry]) => {
      const { claims, ...published } = entry;
      void claims;
      return published;
    });
  return { ordered, objects, conflicted, refinements };
}

function embeddedSource(members) {
  // Emitted in the shape rustfmt produces, not a shape rustfmt would have to
  // fix: a generator whose output fails `cargo fmt --check` makes every
  // regeneration a two-step, and the second step is the one people forget.
  const rows = members
    .map(member =>
      `    (\n        ${JSON.stringify(member)},\n`
      + `        include_bytes!(\n`
      + `            "../../../../../pkg/contracts/accepted/${member}"\n`
      + `        ),\n    ),`
    )
    .join("\n");
  return `//! Generated by \`bun scripts/bundle-accepted-contracts.mjs\`. Do not edit.
//!
//! The bytes of every object named by \`pkg/contracts/accepted/index.json\`,
//! compiled in beside the index so a bundle is one reviewable pair of files in
//! the repository rather than a blob inside another document.

pub(super) const OBJECTS: &[(&str, &[u8])] = &[${rows ? `\n${rows}\n` : ""}];
`;
}

if (basename(process.argv[1] ?? "") === "bundle-accepted-contracts.mjs") main();
