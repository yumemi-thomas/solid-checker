#!/usr/bin/env node
// Runs the checker over every fixture project and records its findings, so a
// refactor can be held to "no finding moved" rather than to "the tests I
// remembered to write still pass".
//
// The hand-maintained counts in tests/rule_quality_process.rs catch a rule
// that stops firing on a file someone remembered to list; they cannot catch a
// finding that moved somewhere nobody listed. This runner can.
//
//   node scripts/coverage.mjs            compare against the snapshots
//   node scripts/coverage.mjs --update   rewrite them
//
// The snapshots live in fixtures/findings-snapshots/, one file per fixture
// project: the project's status, then every finding sorted by location and
// rule, carrying only what a reader can act on: rule, code, kind, severity,
// path, byte span, and whether a fix was offered. Messages and hints are
// deliberately excluded -- rewording a hint should not churn 30 files.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import process from "node:process";

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
      const tsconfig = join(base, entry.name, "tsconfig.json");
      if (existsSync(tsconfig)) found.push({ id: `${group}/${entry.name}`, tsconfig });
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
  for (const group of groups) {
    const base = join(root, "fixtures", group);
    if (!existsSync(base)) continue;
    for (const entry of readdirSync(base, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const stubDirectory = join(base, entry.name, "node_modules", "solid-js");
      if (!existsSync(stubDirectory)) continue;
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
          `${id}: not tracked by git -- add '!${relative(root, join(base, entry.name))}/node_modules/'` +
            ` and its '/**' twin to .gitignore, or the stub is absent in CI`
        );
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
 */
const KEEPS_WORDING = new Set([
  "reactive-ir/dialect-solid-1x",
  "reactive-ir/dialect-solid-2",
  "reactive-ir/import-location",
  "reactive-ir/no-owner-v1",
  "reactive-ir/solid-1x-leftovers"
]);

/**
 * Fixture projects that must stay byte-identical to each other.
 *
 * The dialect pair duplicates application source on purpose. Its declarations
 * differ because each must preserve its published dialect's overloads.
 * If these stop matching, the snapshot diff between them stops meaning "the
 * dialect changed the answer" and starts meaning nothing at all.
 */
const IDENTICAL_SOURCES = [
  {
    projects: ["reactive-ir/dialect-solid-1x", "reactive-ir/dialect-solid-2"],
    files: ["App.tsx", "tsconfig.json"]
  }
];

/**
 * The comparable shape of one finding. Byte offsets rather than line/column so
 * that a fixture edit that shifts a line does not read as a rule change, and
 * repository-relative paths so snapshots do not carry anyone's home directory.
 */
function comparable(finding, keepWording) {
  const location = finding.primaryLocation ?? {};
  const portable = (value) =>
    typeof value === "string" ? value.split(root).join("<ROOT>") : value;
  return {
    rule: finding.rule,
    code: finding.id,
    kind: finding.kind,
    severity: finding.severity,
    path: location.path ? relative(root, location.path) : null,
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

function analyze(tsconfig, keepWording) {
  const output = execFileSync(
    checker,
    ["--format", "json", "--project", tsconfig, ...runtimeArguments(tsconfig)],
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
  const findings = (snapshot.findings ?? []).map((finding) => comparable(finding, keepWording));
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

let changed = 0;
let total = 0;
for (const project of projects) {
  const file = join(snapshots, `${project.id.replace("/", "__")}.json`);
  const actual = `${JSON.stringify(analyze(project.tsconfig, KEEPS_WORDING.has(project.id)), null, 2)}\n`;
  total += JSON.parse(actual).findings.length;

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
if (changed > 0) {
  console.error(`${changed} project(s) differ -- re-run with --update if intended`);
  process.exit(1);
}
