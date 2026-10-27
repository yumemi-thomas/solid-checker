#!/usr/bin/env bun
// Runs the checker over every fixture project and records its findings, so a
// refactor can be held to "no finding moved" rather than to "the tests I
// remembered to write still pass".
//
// The hand-maintained counts in tests/rule_quality_process.rs catch a rule
// that stops firing on a file someone remembered to list; they cannot catch a
// finding that moved somewhere nobody listed. This runner can.
//
//   bun scripts/coverage.mjs            compare against the snapshots
//   bun scripts/coverage.mjs --update   rewrite them
//
// The snapshots live in fixtures/findings-snapshots/, one file per fixture
// project: the project's status, then every finding sorted by location and
// rule, carrying only what a reader can act on: rule, code, kind, severity,
// path, byte span, and whether a fix was offered. Messages and hints are
// deliberately excluded -- rewording a hint should not churn 30 files.

import { execFile } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import process from "node:process";
import { promisify } from "node:util";

import { dialectStubProblems } from "./lib/dialect-stubs.mjs";
import { ancestorChainDigest, hashTree, openGateCache } from "./lib/gate-cache.mjs";
import { gateConcurrency, mapPool } from "./lib/pool.mjs";
import { runCoverageUnit } from "./lib/coverage-cache.mjs";

const run = promisify(execFile);
const root = resolve(import.meta.dirname, "..");
const snapshots = join(root, "fixtures", "findings-snapshots");
const update = process.argv.includes("--update");
// End-to-end differential mode (scripts/typefacts-e2e.mjs): compare only, write
// a machine-readable account of every moved snapshot, and survive a checker
// that fails outright (an unfinished producer does). It must never write a
// snapshot, so it refuses --update.
const reportPath = process.env.SOLID_COVERAGE_REPORT;
if (reportPath && update) {
  console.error("SOLID_COVERAGE_REPORT is compare-only; refusing --update");
  process.exit(2);
}

/** Prefers a packaged binary, falls back to the debug build cargo leaves behind. */
function locate(variable, ...candidates) {
  const override = process.env[variable];
  if (override) return override;
  return candidates.find((candidate) => existsSync(candidate)) ?? candidates[0];
}

const checker = locate(
  "SOLID_CHECKER_BIN",
  join(root, "bin", "solid-checker-rust"),
  join(root, "rust", "target", "debug", "solid-checker-rust")
);
const typefacts = locate("SOLID_TYPEFACTS_BIN", join(root, "bin", "solid-typefacts"));

for (const [name, path] of [
  ["checker", checker],
  ["type facts producer", typefacts]
]) {
  if (!existsSync(path)) {
    console.error(`missing ${name} at ${path} -- run 'make build-rust' first`);
    process.exit(2);
  }
}

/** Every fixture project: a directory holding a tsconfig.json. */
function fixtureProjects() {
  const found = [];
  for (const group of ["reactive-ir", "engine"]) {
    const base = join(root, "fixtures", group);
    if (!existsSync(base)) continue;
    for (const entry of readdirSync(base, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const directory = join(base, entry.name);
      const tsconfig = join(directory, "tsconfig.json");
      if (!existsSync(tsconfig)) continue;
      found.push({
        id: `${group}/${entry.name}`,
        directory,
        tsconfig
      });
      // Variants of one fixture share its `node_modules` (published package
      // bytes copied once) by Node's own lookup: each `variants/<name>/` is a
      // project of its own, whose snapshot id is `<fixture>-<name>`.
      const variants = join(directory, "variants");
      if (existsSync(variants)) {
        for (const variant of readdirSync(variants, { withFileTypes: true })) {
          const variantTsconfig = join(variants, variant.name, "tsconfig.json");
          if (!variant.isDirectory() || !existsSync(variantTsconfig)) continue;
          found.push({
            id: `${group}/${entry.name}-${variant.name}`,
            directory: join(variants, variant.name),
            tsconfig: variantTsconfig
          });
        }
      }
      // No package contract is consumed any more, so a fixture asking for an
      // authorized one would silently test nothing.
      if (existsSync(join(directory, ".solid-checker", "authorize-contract.json"))) {
        throw new Error(`${group}/${entry.name}: authorize-contract.json is retired`);
      }
    }
  }
  found.push(...reproProjects());
  return found.sort((a, b) => a.id.localeCompare(b.id));
}

/**
 * The uncertifiable reproduction set (ADR 0446): projects at any depth
 * (a workspace member sits under its workspace), one snapshot each, whose
 * id joins the path with `-`. They read the set's shared node_modules and,
 * for a workspace member, its sibling manifests, all above the project
 * directory, so the whole set is part of each unit's key (`trees`).
 */
function reproProjects() {
  const base = join(root, "fixtures", "uncertifiable-repro");
  if (!existsSync(base)) return [];
  const found = [];
  const walk = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (!entry.isDirectory() || entry.name === "node_modules" || entry.name.startsWith(".")) continue;
      const path = join(directory, entry.name);
      const tsconfig = join(path, "tsconfig.json");
      if (existsSync(tsconfig)) {
        found.push({
          id: `uncertifiable-repro/${relative(base, path).split("/").join("-")}`,
          directory: path,
          tsconfig,
          trees: [base]
        });
      }
      walk(path);
    }
  };
  walk(base);
  return found;
}

// Dialect stubs decide which catalog each fixture is checked under, so an
// unusable one silently rewrites what a snapshot means. The check and its
// reasoning live in `lib/dialect-stubs.mjs`, where a test can drive its failure
// paths against a throwaway tree.
const stubProblems = dialectStubProblems({ projectRoot: root });
if (stubProblems.length > 0) {
  console.error("fixture dialect stubs are not usable:");
  for (const problem of stubProblems) console.error(`  ${problem}`);
  process.exit(2);
}

/**
 * Projects whose snapshots keep the message and hint text.
 *
 * The exception exists because for these the wording *is* the behaviour under
 * test: a dialect-specific diagnostic that quotes the wrong signature or names
 * a component the dialect does not have is exactly the failure they are here
 * to catch, and excluding the text would leave them unable to fail for the
 * reason they exist. Everywhere else the text stays out, so rewording a hint
 * still does not churn 30 files.
 *
 * The two `jsx-census-gap` projects are here for the same reason in a different
 * shape. Their claim is not "a finding appears at this span with this `kind`":
 * an uncertifiable SC1001 whose message still reads "which does not track; the
 * read sees the current value once and never updates" asserts exactly the thing
 * the census hole leaves unproven, and no other field in the snapshot can tell
 * that apart from one that names the missing compiler fact. Without the text,
 * `strict_read_message`'s census branch could revert and every gate would stay
 * green — verified by deleting the branch, which now fails both projects here.
 * The evidence chain is in no snapshot at all, so the matching evidence
 * sentence (`untracked_evidence_sentence`) is pinned by unit tests in
 * `rust/crates/solid-reactive-ir/src/findings.rs` instead.
 */
const KEEPS_WORDING = new Set([
  "reactive-ir/dialect-solid-2",
  "reactive-ir/jsx-census-gap-solid-2",
  "reactive-ir/jsx-void-child-divergence-solid-2",
  // Not a 1.x fixture despite the name: it is a 2.0 project importing four
  // names 2.0 removed, and the removed-export *message* is the whole claim.
  "reactive-ir/retired-1x-spellings",
  // SC8014's two messages differ only in wording: the one-parameter arrow gets
  // a `<For>` rewrite and a fix, and everything else is told which component to
  // reach for instead. That second message named `<Index />` until 2026-09-16
  // -- a component Solid 2.0 does not export -- and nothing pinned it.
  "reactive-ir/ported-structure-v2",
  // SC1001's read context named `createEffect` for every effect apply until
  // 2026-09-26, `createRenderEffect`'s included; only the text can say which
  // primitive a read's apply belongs to.
  "reactive-ir/render-effect-apply-root",
  // ADR 0352: the SC4001 wording names the NoOwnerError throw, which is the
  // claim; the package case also pins that the finding is placed at the call.
  "reactive-ir/context-read-owner",
  "reactive-ir/context-read-package",
  // ADR 0352: the SC4002 wording names the ContextNotFoundError throw, the claim.
  "reactive-ir/context-provider-closed"
]);

/**
 * Fixture projects that must stay byte-identical to each other.
 *
 * The dialect pair duplicates application source on purpose. Its declarations
 * differ because each must preserve its published dialect's overloads.
 * If these stop matching, the snapshot diff between them stops meaning "the
 * dialect changed the answer" and starts meaning nothing at all.
 */
// Both entries were dialect pairs, and both lost their 1.x half when the 1.x
// dialect was retired (ADR 0110). The list is kept rather than deleted: it is
// the mechanism for "these two projects must stay byte-identical or their
// snapshot diff stops meaning anything", and the next divergence pair needs it.
const IDENTICAL_SOURCES = [
  // ADR 0295: the same project and package with and without a declared
  // Window realm; only `.solid-checker/runtime.json` (and the README) differ.
  {
    projects: [
      "reactive-ir/dependency-source-window-realm",
      "reactive-ir/dependency-source-window-undeclared"
    ],
    files: [
      "App.tsx",
      "solid-js.d.ts",
      "tsconfig.json",
      "node_modules/solid-js/package.json",
      "node_modules/solid-js/dist/solid.js",
      "node_modules/dep-query/package.json",
      "node_modules/dep-query/dist/index.d.ts",
      "node_modules/dep-query/src/index.js",
      "node_modules/dep-query/src/early.js",
      "node_modules/dep-query/src/cyclic.js",
      "node_modules/dep-query/src/cycle-back.js",
      // Review round 1 of ADR 0295.
      "Review.tsx",
      "node_modules/dep-query/src/tdz.js",
      "node_modules/dep-query/src/hoisted.js",
      "node_modules/dep-array/package.json",
      "node_modules/dep-array/dist/index.d.ts",
      "node_modules/dep-array/src/index.js",
      "node_modules/dep-bridge/package.json",
      "node_modules/dep-bridge/cycle.js",
      "node_modules/dep-bridge/leaf.js",
      "node_modules/dep-self/package.json",
      "node_modules/dep-self/dist/index.d.ts",
      "node_modules/dep-self/src/index.js",
      "node_modules/dep-self/node_modules/dep-self/package.json",
      "node_modules/dep-self/node_modules/dep-self/src/index.js"
    ]
  }
];

/**
 * The comparable shape of one finding. Byte offsets rather than line/column so
 * that a fixture edit that shifts a line does not read as a rule change, and
 * repository-relative paths so snapshots do not carry anyone's home directory.
 */
function comparable(finding, keepWording) {
  const rebase = (path) => relative(root, path);
  const location = finding.primaryLocation ?? {};
  const portable = (value) =>
    typeof value === "string" ? value.split(root).join("<ROOT>") : value;
  return {
    rule: finding.rule,
    code: finding.id,
    kind: finding.kind,
    severity: finding.severity,
    path: location.path ? rebase(location.path) : null,
    start: location.startByte ?? null,
    end: location.endByte ?? null,
    fixes: Array.isArray(finding.fixes) ? finding.fixes.length : 0,
    ...(keepWording
      ? { message: portable(finding.message), hint: portable(finding.hint) }
      : {})
  };
}

/** Reports pairs of fixture projects whose shared sources have drifted apart. */
function driftedSources() {
  const drifted = [];
  for (const { projects, files } of IDENTICAL_SOURCES) {
    const [first, ...rest] = projects;
    if (!existsSync(join(root, "fixtures", first))) continue;
    for (const other of rest) {
      for (const file of files) {
        const a = join(root, "fixtures", first, file);
        const b = join(root, "fixtures", other, file);
        if (!existsSync(a) || !existsSync(b) || readFileSync(a, "utf8") !== readFileSync(b, "utf8")) {
          drifted.push(`${relative(root, a)} and ${relative(root, b)}`);
        }
      }
    }
  }
  return drifted;
}

function runtimeArguments(tsconfig) {
  const metadata = join(dirname(tsconfig), ".solid-checker", "runtime.json");
  if (!existsSync(metadata)) return [];
  const runtime = JSON.parse(readFileSync(metadata, "utf8"));
  if (runtime == null || typeof runtime !== "object" || Array.isArray(runtime)) {
    throw new Error(`${metadata}: runtime metadata must be an object`);
  }
  const args = [];
  for (const [key, flag] of [
    ["target", "--runtime-target"],
    ["build", "--runtime-build"],
    // ADR 0295: the declared realm of the dependency sources (`window`).
    ["realm", "--runtime-realm"],
    ["rendering", "--rendering"],
    ["programBoundary", "--program-boundary"]
  ]) {
    if (runtime[key] !== undefined) args.push(flag, runtime[key]);
  }
  for (const condition of runtime.conditions ?? []) args.push("--runtime-condition", condition);
  for (const transform of runtime.frameworkTransforms ?? []) {
    args.push("--framework-transform", transform);
  }
  // ADR 0284: installed packages whose runtime source is analyzed as project code.
  for (const specifier of runtime.dependencySources ?? []) {
    args.push("--dependency-source", specifier);
  }
  // ADR 0297: a declared target and build analyze every Solid package from
  // source by default. The opt-out is retired.
  if (runtime.dependencySource !== undefined) {
    throw new Error(`${metadata}: dependencySource is retired with package contracts`);
  }
  return args;
}

async function analyze(tsconfig, keepWording) {
  if (!reportPath) return analyzeUnguarded(tsconfig, keepWording);
  try {
    return await analyzeUnguarded(tsconfig, keepWording);
  } catch (error) {
    return {
      status: "checker-failed",
      findings: [],
      failure: {
        exitCode: error.code ?? null,
        stderr: String(error.stderr ?? error.message).trim().slice(0, 600)
      }
    };
  }
}

async function analyzeUnguarded(tsconfig, keepWording) {
  const { stdout: output } = await run(
    checker,
    [
      "--format",
      "json",
      "--project",
      tsconfig,
      ...runtimeArguments(tsconfig)
    ],
    {
      cwd: root,
      encoding: "utf8",
      maxBuffer: 256 * 1024 * 1024,
      env: {
        ...process.env,
        // One retained daemon per fixture would outlive the gate by its idle
        // limit, hundreds at once; a gate run is one-shot.
        SOLID_CHECKER_DAEMON: "0",
        SOLID_TYPEFACTS_BIN: typefacts
      }
    }
  );
  const snapshot = JSON.parse(output);
  const findings = (snapshot.findings ?? []).map((finding) =>
    comparable(finding, keepWording)
  );
  findings.sort(
    (a, b) =>
      (a.path ?? "").localeCompare(b.path ?? "") ||
      (a.start ?? 0) - (b.start ?? 0) ||
      a.code.localeCompare(b.code) ||
      a.rule.localeCompare(b.rule)
  );
  // status is part of the contract too: a change that keeps every finding but
  // flips the verdict is still a change.
  return { status: snapshot.status, findings };
}

mkdirSync(snapshots, { recursive: true });

const projects = fixtureProjects();
if (projects.length === 0) {
  console.error("no fixture projects found under fixtures/");
  process.exit(2);
}

// A fixture project is self-contained in the ways that are easy to check: it
// holds its own `tsconfig.json`, its own sources, its own `node_modules/solid-js`
// dialect stub, and its own optional `.solid-checker/runtime.json`. None of them
// `extends` a shared config or reaches outside the directory.
//
// It is *not* self-contained in one way, and the key has to say so. Dialect
// selection walks ancestors: `resolved_solid_version`
// (rust/crates/solid-facts-backend/src/dialect.rs) climbs `start.ancestors()`
// unbounded, past this repository, to `/`, taking the nearest
// `node_modules/solid-js/package.json` it finds. Roughly half these projects
// ship no stub and rely on there being none above them -- which is true of the
// checkout and says nothing about the directory containing it, or about `$HOME`.
// So the absence of an ancestor stub is an input, and `ancestorChainDigest`
// puts the whole chain in the key: a stray `bun install solid-js` one directory
// up now misses instead of replaying pre-install findings while `checkDialectStubs`
// (the thing that catches a substituted dialect) never runs.
//
// These keys cover explicit-target units. No-target units bypass result reuse:
// inference additionally observes ancestors/dependencies of the materialized
// project, including absent PostCSS/config/package candidates, that this key
// does not enumerate. A native daemon's revalidation cannot protect a gate hit.
const cache = openGateCache({
  gate: "coverage",
  scriptPath: import.meta.filename,
  binaries: [checker, typefacts, `${typefacts}.buildinfo`]
});
const concurrency = gateConcurrency();

// A thunk, not an array: the digests below are of mutable state, so the cache
// re-evaluates them after the checker has run and refuses to store a unit whose
// tree moved underneath it. See `openGateCache().run`.
const unitParts = (project) => () => [
  `project:${project.id}`,
  `wording:${KEEPS_WORDING.has(project.id)}`,
  hashTree(project.directory),
  ...(project.trees ?? []).map((tree) => hashTree(tree)),
  hashTree(join(root, "fixtures/reactive-ir/node_modules/@solidjs/vite-plugin")),
  ancestorChainDigest(project.directory, "node_modules/solid-js/package.json"),
  // The installation review reads two more manifests the same way
  // (`resolved_releases` in dialect.rs): `@solidjs/web` from the project, and
  // `@solidjs/signals` from the installed `solid-js`'s directory, which for a
  // stub-less project or an ancestor stub lies on this same chain -- either
  // hoisted beside `solid-js` or nested under it.
  ancestorChainDigest(project.directory, "node_modules/@solidjs/signals/package.json"),
  ancestorChainDigest(
    project.directory,
    "node_modules/solid-js/node_modules/@solidjs/signals/package.json"
  ),
  ancestorChainDigest(project.directory, "node_modules/@solidjs/web/package.json")
];

const computed = await mapPool(
  projects,
  (project) =>
    runCoverageUnit(
      cache,
      unitParts(project),
      () => analyze(project.tsconfig, KEEPS_WORDING.has(project.id)),
      runtimeArguments(project.tsconfig)
    ),
  { concurrency }
);

// Comparison runs fresh, in project order, whether the analysis was replayed
// or not: the snapshot on disk is never part of the cache key, so editing one
// needs no cache awareness and a mismatch still fails on a warm cache.
let changed = 0;
let total = 0;
const moved = [];
/** What moved in one snapshot, as sets of finding keys. */
function movedEntry(id, expectedText, value) {
  const expected = JSON.parse(expectedText);
  const key = (finding) => JSON.stringify(finding);
  const was = new Set(expected.findings.map(key));
  const now = new Set(value.findings.map(key));
  let state = "moved";
  if (value.status === "checker-failed") state = "checker-failed";
  else if (expectedText === `${JSON.stringify(value, null, 2)}\n`) state = "unchanged";
  return {
    project: id,
    state,
    statusWas: expected.status,
    statusNow: value.status,
    findingsWas: expected.findings.length,
    findingsNow: value.findings.length,
    removed: expected.findings.filter((finding) => !now.has(key(finding))).slice(0, 10),
    added: value.findings.filter((finding) => !was.has(key(finding))).slice(0, 10),
    failure: value.failure
  };
}
for (const [index, project] of projects.entries()) {
  const file = join(snapshots, `${project.id.replace("/", "__")}.json`);
  const actual = `${JSON.stringify(computed[index].value, null, 2)}\n`;
  total += computed[index].value.findings.length;

  if (update) {
    writeFileSync(file, actual);
    continue;
  }
  if (!existsSync(file)) {
    console.error(`no snapshot for ${project.id} -- run with --update`);
    changed += 1;
    continue;
  }
  const expected = readFileSync(file, "utf8");
  if (reportPath) moved.push(movedEntry(project.id, expected, computed[index].value));
  if (expected !== actual) {
    console.error(`findings moved: ${project.id}`);
    const expectedLines = expected.split("\n");
    const actualLines = actual.split("\n");
    for (let i = 0; i < Math.max(expectedLines.length, actualLines.length); i += 1) {
      if (expectedLines[i] !== actualLines[i]) {
        console.error(`  line ${i + 1}`);
        console.error(`    was: ${expectedLines[i] ?? "<end>"}`);
        console.error(`    now: ${actualLines[i] ?? "<end>"}`);
        break;
      }
    }
    changed += 1;
  }
}

for (const pair of driftedSources()) {
  console.error(`fixture sources that must match have drifted: ${pair}`);
  changed += 1;
}

// A snapshot whose project no longer exists is a silent hole in the "no
// finding moved" guarantee: every finding it pinned vanished from coverage
// without anything failing. Deleting a fixture project must be as loud as
// changing one — and `--update` prunes the orphan instead.
const expectedFiles = new Set(projects.map((project) => `${project.id.replace("/", "__")}.json`));
for (const entry of readdirSync(snapshots)) {
  if (!entry.endsWith(".json") || expectedFiles.has(entry)) continue;
  if (update) {
    rmSync(join(snapshots, entry));
    console.log(`pruned orphaned snapshot ${entry}`);
  } else {
    console.error(`orphaned snapshot ${entry} -- its fixture project is gone; run with --update to prune`);
    changed += 1;
  }
}

if (reportPath) {
  const states = {};
  for (const entry of moved) states[entry.state] = (states[entry.state] ?? 0) + 1;
  writeFileSync(
    reportPath,
    `${JSON.stringify({ version: 1, projects: moved.length, states, moved }, null, 2)}\n`
  );
  console.log(`snapshot report: ${reportPath}`);
}
const verb = update ? "recorded" : "compared";
console.log(`${verb} ${projects.length} fixture projects, ${total} findings`);
console.log(`${cache.summary()}; concurrency ${concurrency}`);
console.log(`inference cache bypass: ${computed.filter((unit) => unit.inferenceCacheBypassed).length} unit(s)`);
if (changed > 0) {
  console.error(`${changed} project(s) differ -- re-run with --update if intended`);
  process.exit(1);
}
