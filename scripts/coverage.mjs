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

import { execFile, execFileSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import process from "node:process";
import { promisify } from "node:util";

import { ancestorChainDigest, hashTree, openGateCache } from "./lib/gate-cache.mjs";
import { gateConcurrency, mapPool } from "./lib/pool.mjs";

const run = promisify(execFile);
const root = resolve(import.meta.dirname, "..");
const snapshots = join(root, "fixtures", "findings-snapshots");
const update = process.argv.includes("--update");

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

// A fixture that wants to be analyzed against an *accepted* contract is run from
// a scratch copy, because the receipt binds the absolute importer path the
// consumer itself computes -- an authorized tree is bound to where it sits and
// cannot be committed. This tool mints that receipt; it is built alongside the
// checker by every target that builds one, and is never packaged.
//
// Looked for beside the checker first, for that reason: a `verify`-profile
// checker paired with a stale `debug` authorizer would be two different builds
// deciding one answer.
const authorizeTool = locate(
  "SOLID_CONTRACT_AUTHORIZE_BIN",
  join(dirname(checker), "solid-contract-authorize"),
  join(root, "rust", "target", "debug", "solid-contract-authorize")
);
const authorizationScratch = join(root, "rust", "target", "fixture-authorization");

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
        tsconfig,
        authorizes: existsSync(join(directory, ".solid-checker", "authorize-contract.json"))
      });
    }
  }
  return found.sort((a, b) => a.id.localeCompare(b.id));
}

/**
 * Holds every fixture dialect stub to being present, parseable, and tracked.
 *
 * Dialect selection follows the nearest `node_modules/solid-js/package.json`
 * above the project, and a stub that is missing, empty, or unparseable falls
 * back silently to the 2.0 default. Two ways that happens leave no other
 * trace: an empty `node_modules/solid-js/` directory (git cannot record an
 * empty directory, so the stub never arrives), and a stub with no
 * `.gitignore` exception under the repository-wide `**\/node_modules/` rule
 * (present locally, absent in CI). Either one turns a 1.x fixture into a 2.0
 * fixture whose snapshot then records the wrong catalog as if intended.
 *
 * `eslint-plugin-corpus-v1` shipped the first shape and `solid-reexport` the
 * second, so this is a check, not a hypothetical.
 */
function checkDialectStubs() {
  const tracked = new Set(
    execFileSync("git", ["ls-files", "-z", "fixtures"], { cwd: root, encoding: "utf8" })
      .split("\0")
      .filter(Boolean)
  );
  const problems = [];
  const groups = ["reactive-ir", "engine", "package-contracts", "ownership-cases", "partial-audit"];
  /**
   * Every `node_modules/solid-js` stub at any depth below `directory`, skipping
   * the inside of `node_modules` trees themselves. A fixture may hold a nested
   * package (`closed-domain-probe-gate/primitive-consumer/`) with a stub of its
   * own, and a stub one level down is exactly as load-bearing for dialect
   * selection -- and exactly as silently absent in CI without its `.gitignore`
   * exception -- as one at the fixture root.
   */
  function* stubDirectories(directory) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const path = join(directory, entry.name);
      if (entry.name === "node_modules") {
        const stub = join(path, "solid-js");
        if (existsSync(stub)) yield { stub, fixture: directory };
        continue;
      }
      yield* stubDirectories(path);
    }
  }
  for (const group of groups) {
    const base = join(root, "fixtures", group);
    if (!existsSync(base)) continue;
    for (const entry of readdirSync(base, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      for (const { stub: stubDirectory, fixture } of stubDirectories(join(base, entry.name))) {
        const manifest = join(stubDirectory, "package.json");
        const id = relative(root, manifest);
        if (!existsSync(manifest)) {
          problems.push(`${id}: missing -- the fixture falls back to the 2.0 default dialect`);
          continue;
        }
        let version;
        try {
          version = JSON.parse(readFileSync(manifest, "utf8")).version;
        } catch (error) {
          problems.push(`${id}: unparseable (${error.message})`);
          continue;
        }
        if (typeof version !== "string" || version === "") {
          problems.push(`${id}: no "version" -- dialect selection cannot resolve it`);
        }
        if (!tracked.has(id)) {
          problems.push(
            `${id}: not tracked by git -- add '!${relative(root, fixture)}/node_modules/'` +
              ` and its '/**' twin to .gitignore, or the stub is absent in CI`
          );
        }
      }
    }
  }
  if (problems.length > 0) {
    console.error("fixture dialect stubs are not usable:");
    for (const problem of problems) console.error(`  ${problem}`);
    process.exit(2);
  }
}

checkDialectStubs();

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
  "reactive-ir/ported-structure-v2"
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
const IDENTICAL_SOURCES = [];

/**
 * The comparable shape of one finding. Byte offsets rather than line/column so
 * that a fixture edit that shifts a line does not read as a rule change, and
 * repository-relative paths so snapshots do not carry anyone's home directory.
 */
function comparable(finding, keepWording, rebase = (path) => relative(root, path)) {
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
    ["rendering", "--rendering"],
    ["programBoundary", "--program-boundary"]
  ]) {
    if (runtime[key] !== undefined) args.push(flag, runtime[key]);
  }
  for (const condition of runtime.conditions ?? []) args.push("--runtime-condition", condition);
  for (const transform of runtime.frameworkTransforms ?? []) {
    args.push("--framework-transform", transform);
  }
  return args;
}

/**
 * Materializes a fixture that asked for an accepted contract, authorizes its
 * own hand-written resolution, and returns where to analyze it.
 *
 * Three things here are the point rather than incidental mechanics:
 *
 * - **A copy.** The receipt binds the absolute importer path, so authorizing in
 *   place would write a machine-bound receipt into the tree, move the directory
 *   digest underneath the gate cache, and race the other projects.
 * - **Trust out of band.** The configuration is written *beside* the copy and
 *   passed as a flag. A project cannot nominate its own issuer, so the same
 *   authorized tree analyzed without the flag is refused outright -- which is
 *   what keeps a fixed fixture signing key from being a forgery, and is pinned
 *   by `an_authorized_catalog_is_refused_without_the_trust_configuration`.
 * - **The stub is required.** Dialect selection takes the nearest
 *   `node_modules/solid-js/package.json` above the project, and a copy sits at a
 *   different depth than the fixture. A fixture carrying its own stub decides
 *   its dialect wherever it is analyzed; one relying on an ancestor would
 *   silently change catalog when copied.
 */
function materializeAuthorized(project) {
  if (!existsSync(authorizeTool)) {
    console.error(
      `${project.id} asks for an accepted contract but ${relative(root, authorizeTool)}` +
        ` is missing -- run 'make build-checker-debug'`
    );
    process.exit(2);
  }
  if (!existsSync(join(project.directory, "node_modules", "solid-js", "package.json"))) {
    console.error(
      `${project.id} asks for an accepted contract but ships no node_modules/solid-js stub --` +
        ` a copied fixture cannot inherit a dialect from its ancestors`
    );
    process.exit(2);
  }
  const base = join(authorizationScratch, project.id.replace("/", "__"));
  const destination = join(base, "project");
  rmSync(base, { recursive: true, force: true });
  mkdirSync(base, { recursive: true });
  cpSync(project.directory, destination, { recursive: true });
  const trust = join(base, "trust.json");
  execFileSync(authorizeTool, ["--project", destination, "--trust-output", trust], {
    encoding: "utf8"
  });
  // The tool canonicalizes its own `--project`, and the receipt binds what it
  // canonicalized; the checker must be pointed at the same spelling.
  return {
    directory: realpathSync(destination),
    trust,
    id: relative(root, project.directory).split(sep).join("/")
  };
}

async function analyze(tsconfig, keepWording, authorized) {
  const { stdout: output } = await run(
    checker,
    [
      "--format",
      "json",
      "--project",
      tsconfig,
      ...(authorized ? ["--receipt-trust-configuration", authorized.trust] : []),
      ...runtimeArguments(tsconfig)
    ],
    {
      cwd: root,
      encoding: "utf8",
      maxBuffer: 256 * 1024 * 1024,
      env: {
        ...process.env,
        SOLID_TYPEFACTS_BIN: typefacts
      }
    }
  );
  const snapshot = JSON.parse(output);
  // An authorized project is analyzed from scratch space, so its findings name
  // paths that exist nowhere in the repository. Spell them as the fixture the
  // snapshot is about.
  const rebase = authorized
    ? (path) => join(authorized.id, relative(authorized.directory, path))
    : undefined;
  const findings = (snapshot.findings ?? []).map((finding) =>
    comparable(finding, keepWording, rebase)
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
// With that added, a project's findings are a function of exactly its tree, the
// dialect-selection chain above it, the two binaries, and the environment --
// which is what makes running the 83 of them concurrently sound.
// The authorization tool is an input wherever a project asks for an accepted
// contract: it decides what the analyzed catalog says. It joins the key only
// when some project asks, so a corpus with none of them is not invalidated by a
// tool it never runs -- and `materializeAuthorized` exits rather than skipping
// when a project asks and the tool is absent.
const cache = openGateCache({
  gate: "coverage",
  scriptPath: import.meta.filename,
  binaries: projects.some((project) => project.authorizes)
    ? [checker, typefacts, `${typefacts}.buildinfo`, authorizeTool]
    : [checker, typefacts, `${typefacts}.buildinfo`]
});
const concurrency = gateConcurrency();

// A thunk, not an array: the digests below are of mutable state, so the cache
// re-evaluates them after the checker has run and refuses to store a unit whose
// tree moved underneath it. See `openGateCache().run`.
const unitParts = (project) => () => [
  `project:${project.id}`,
  `wording:${KEEPS_WORDING.has(project.id)}`,
  hashTree(project.directory),
  ancestorChainDigest(project.directory, "node_modules/solid-js/package.json")
];

const computed = await mapPool(
  projects,
  (project) =>
    cache.run(unitParts(project), () => {
      const authorized = project.authorizes ? materializeAuthorized(project) : undefined;
      const tsconfig = authorized
        ? join(authorized.directory, "tsconfig.json")
        : project.tsconfig;
      return analyze(tsconfig, KEEPS_WORDING.has(project.id), authorized);
    }),
  { concurrency }
);

// Comparison runs fresh, in project order, whether the analysis was replayed
// or not: the snapshot on disk is never part of the cache key, so editing one
// needs no cache awareness and a mismatch still fails on a warm cache.
let changed = 0;
let total = 0;
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

const verb = update ? "recorded" : "compared";
console.log(`${verb} ${projects.length} fixture projects, ${total} findings`);
console.log(`${cache.summary()}; concurrency ${concurrency}`);
if (changed > 0) {
  console.error(`${changed} project(s) differ -- re-run with --update if intended`);
  process.exit(1);
}
