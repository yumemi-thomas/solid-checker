#!/usr/bin/env bun
// Regenerates the compiled-in accepted-contract tier from a certification run.
//
//   bun scripts/bundle-accepted-contracts.mjs --run <run.json>
//   bun scripts/bundle-accepted-contracts.mjs --catalogs <DIR> --dry-run
//
// The inputs are the published catalogs a `--keep-temp` ecosystem run left
// behind -- the same artifacts `contract-coverage-census.mjs` reads, so the
// measurement and the delivery come from one run rather than two. For each
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

function parseArguments(argv) {
  const options = { run: "", catalogs: "", dryRun: false, allEntrypoints: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--run") {
      options.run = argv[++index] ?? fail("--run needs a path");
    } else if (argument === "--catalogs") {
      options.catalogs = argv[++index] ?? fail("--catalogs needs a directory");
    } else if (argument === "--dry-run") {
      options.dryRun = true;
    } else if (argument === "--all-entrypoints") {
      options.allEntrypoints = true;
    } else {
      fail(`unknown argument ${argument}`);
    }
  }
  if (!options.run && !options.catalogs) {
    fail("one of --run <run.json> or --catalogs <DIR> is required");
  }
  return options;
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

/** The one key a bundle is unique under: an artifact identity, spelled out. */
function bundleKey(entry) {
  return [
    entry.packageName,
    entry.packageVersion,
    entry.requestedEntrypoint,
    entry.exportConditions.join("+")
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
function claimsFingerprint(documentText) {
  const document = JSON.parse(documentText);
  const claims = [];
  for (const [entrypoint, value] of Object.entries(document.entrypoints ?? {})) {
    for (const artifactCase of value.cases ?? [value]) {
      for (const [name, reference] of Object.entries(artifactCase?.exports ?? {})) {
        claims.push([
          entrypoint,
          name,
          JSON.stringify(document.summaries?.[reference] ?? null)
        ]);
      }
    }
  }
  claims.sort(([leftEntry, leftName], [rightEntry, rightName]) =>
    leftEntry.localeCompare(rightEntry) || leftName.localeCompare(rightName)
  );
  return sha256(JSON.stringify(claims));
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  if (!existsSync(BUNDLER)) {
    fail(`${BUNDLER} does not exist; run \`make build-checker-debug\``);
  }
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
      fail(`${directory} does not exist`);
    }
  }

  const bundles = new Map();
  const objects = new Map();
  const refused = [];
  const conflicted = new Set();
  for (const catalog of roots.flatMap(publishedCatalogs)) {
    const trust = trustConfigurationFor(catalog);
    if (!trust) {
      refused.push([catalog, "no trust configuration beside or above the catalog"]);
      continue;
    }
    let result;
    try {
      result = bundleCatalog(catalog, trust);
    } catch (error) {
      refused.push([catalog, String(error.stderr ?? error.message ?? "").trim()]);
      continue;
    }
    for (const entry of result.bundles) {
      // An entrypoint reached only through a `./*` wildcard answers no import a
      // consumer writes, and the census does not count it either. Bundling one
      // would carry bytes nobody can reach. `--all-entrypoints` keeps them.
      if (!options.allEntrypoints && !nameableEntrypoint(entry.requestedEntrypoint)) continue;
      const key = bundleKey(entry);
      const claims = claimsFingerprint(result.objects[entry.document]);
      const previous = bundles.get(key);
      if (previous && previous.claims !== claims) {
        // Two certifications of one published artifact that do not agree.
        // Neither may be applied -- which one describes the bytes is exactly
        // the question this cannot answer -- so the artifact is dropped and
        // named. Dropping one key must not stop the other bundles: a corpus
        // run reaches many packages, and one disagreement used to abort the
        // whole generation.
        conflicted.add(key);
        continue;
      }
      // Deterministic among documents that agree, so regenerating the same run
      // reproduces the same bytes whatever order the catalogs were walked in.
      if (!previous || entry.documentDigest < previous.documentDigest) {
        bundles.set(key, { ...entry, claims });
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
  const index = {
    format: "solid-checker-accepted-contract-bundle-index",
    bundleIndexVersion: 1,
    bundles: ordered
  };

  for (const [catalog, reason] of refused) {
    console.error(`  refused ${relative(REPOSITORY, catalog)}: ${reason.split("\n")[0]}`);
  }
  for (const key of [...conflicted].sort()) {
    console.error(`  dropped ${key}: two certifications of it do not agree`);
  }
  const packages = new Set(ordered.map(entry => entry.packageName));
  console.log(`${ordered.length} bundle(s) over ${packages.size} package(s)`);
  for (const entry of ordered) {
    console.log(`  ${entry.packageName}@${entry.packageVersion} ${entry.requestedEntrypoint}`);
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
