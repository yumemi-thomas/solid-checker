#!/usr/bin/env bun
// The uncertifiable reproduction set's budget (ADR 0404).
//
// fixtures/uncertifiable-repro stands in for the 13 GB corpus a precision
// slice used to be measured on, so it has to stay small enough to run in
// seconds on any machine and honest enough that its results mean what the
// corpus's meant. This gate holds both: few projects, one short file per cause
// or shape, a source size per kind of case, and a shared node_modules that
// holds only the published bytes its manifest lists. Violation and clean cases
// (from the pre-release run, ADR 0404) have budget lines of their own, so they
// never crowd out the uncertifiable ones. Raising a budget is a deliberate edit
// here, with the reason beside it, as in scripts/doc-budget.mjs.
//
//   bun scripts/repro-budget.mjs        check, exit 1 on any violation
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
export const SET = "fixtures/uncertifiable-repro";

export const BUDGETS = {
  /**
   * Projects (directories holding a tsconfig.json). Raised from 8 to 9 for
   * `other-rules`, the one shared project of the violation and clean cases of
   * rules with no project of their own (SC1004, SC3001, SC8014, SC8015, SC9015).
   */
  projects: 9,
  /** Source files of one case: one, or two when the cause is cross-file. */
  filesPerCase: 2,
  /** Lines of one source file. */
  fileLines: 40,
  /** Per kind of case: how many source files, and how many lines together. */
  kinds: {
    // Raised from 60 files and 1_500 lines when every cause of the rc.17
    // corpus, not only those of 20 results or more, was to be reproduced:
    // 149 files and 2_849 lines at 139 cases (97.4% of the corpus's results).
    // Raised from 160 and 3_000 when mixed labels were split into one case per
    // mechanism (ADR 0406): 177 files and 3_390 lines at 167 cases.
    uncertifiable: { files: 180, lines: 3_400 },
    violation: { files: 16, lines: 400 },
    clean: { files: 16, lines: 400 },
  },
  /** Bytes of the shared node_modules. */
  nodeModulesBytes: 5_000_000,
};

/**
 * How many distinct corpus sites of an uncertifiable case are read from
 * source before its label counts as verified: all of them when there are
 * fewer.
 */
export const VERIFICATION_SAMPLE = 5;

/** The verdicts a source read gives a sampled site. */
export const VERDICTS = ["same", "different", "unsure"];

/**
 * ADR 0404's verified weight: a case stands for the distinct sites of its
 * label only when every sampled site reads, from source, as the mechanism its
 * files reproduce; one other reading makes the label a grouping and nothing
 * more (`verifiedWeight` 0), until the bucket is split.
 */
export function verifiedWeight(entry) {
  const samples = entry.verification?.samples ?? [];
  const needed = Math.min(VERIFICATION_SAMPLE, entry.sites ?? 0);
  return needed > 0 && samples.length >= needed && samples.every((sample) => sample.verdict === "same") ? entry.sites : 0;
}

/** The status a case of each kind may have today. */
export const STATUSES = {
  // A slice that certifies an uncertifiable case moves it to `certified`.
  uncertifiable: ["uncertifiable", "certified"],
  violation: ["violation"],
  clean: ["certified"],
};

const SOURCE = /\.(tsx?|jsx?)$/;

/** Every project of the set: a directory holding a tsconfig.json, outside node_modules. */
export function projectsOf(root, prefix = "") {
  const found = [];
  for (const entry of readdirSync(join(root, prefix), { withFileTypes: true })) {
    if (!entry.isDirectory() || entry.name === "node_modules" || entry.name.startsWith(".")) continue;
    const path = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (existsSync(join(root, path, "tsconfig.json"))) found.push(path);
    found.push(...projectsOf(root, path));
  }
  return found.sort();
}

function filesUnder(root, directory, skip = () => false) {
  const found = [];
  for (const entry of readdirSync(join(root, directory), { withFileTypes: true })) {
    const path = directory ? `${directory}/${entry.name}` : entry.name;
    if (entry.isDirectory()) {
      if (!skip(entry.name)) found.push(...filesUnder(root, path, skip));
    } else found.push(path);
  }
  return found;
}

/**
 * What the gate reads, from disk: the projects, every source file outside
 * node_modules with its line count, every node_modules file with its size and
 * sha256, the case table and the package manifest. Separate from the checks
 * so a test can hand `violations` a state that is not this repository's.
 */
export function measure(root = join(ROOT, SET)) {
  const sources = Object.fromEntries(
    filesUnder(root, "", (name) => name === "node_modules")
      .filter((path) => SOURCE.test(path))
      .map((path) => [path, readFileSync(join(root, path), "utf8").replace(/\n$/, "").split("\n").length]),
  );
  const modules = Object.fromEntries(
    filesUnder(root, "node_modules").map((path) => {
      const bytes = readFileSync(join(root, path));
      return [path, { size: statSync(join(root, path)).size, sha256: createHash("sha256").update(bytes).digest("hex") }];
    }),
  );
  return {
    projects: projectsOf(root),
    sources,
    modules,
    table: JSON.parse(readFileSync(join(root, "cases.json"), "utf8")),
    packages: JSON.parse(readFileSync(join(root, "packages.json"), "utf8")),
  };
}

/** Every reason the measured set breaks its budget or its contract, as printable lines. */
export function violations(state, { tracked = null, manifest = null } = {}) {
  const found = [];
  if (state.projects.length > BUDGETS.projects) {
    found.push(`${state.projects.length} projects, budget ${BUDGETS.projects}: ${state.projects.join(", ")}`);
  }
  for (const [path, lines] of Object.entries(state.sources)) {
    if (lines > BUDGETS.fileLines) found.push(`${path}: ${lines} lines, budget ${BUDGETS.fileLines} per file`);
  }

  // The table: every case names a project, its files and what it stands for,
  // and every source file belongs to exactly one case.
  const owner = new Map();
  const ids = new Set();
  for (const entry of state.table.cases ?? []) {
    const where = `cases.json ${entry.id ?? "(no id)"}`;
    if (!entry.id || ids.has(entry.id)) found.push(`${where}: missing or repeated id`);
    ids.add(entry.id);
    if (!state.projects.includes(entry.project)) found.push(`${where}: project ${entry.project} is not a project of the set`);
    if (!Array.isArray(entry.files) || entry.files.length === 0 || entry.files.length > BUDGETS.filesPerCase) {
      found.push(`${where}: ${entry.files?.length ?? 0} files, budget 1 to ${BUDGETS.filesPerCase}`);
    }
    for (const file of entry.files ?? []) {
      const path = `${entry.project}/${file}`;
      if (!(path in state.sources)) found.push(`${where}: ${path} is not a source file of the set`);
      if (owner.has(path)) found.push(`${where}: ${path} also belongs to ${owner.get(path)}`);
      owner.set(path, entry.id);
    }
    if (!(entry.kind in STATUSES)) {
      found.push(`${where}: kind ${entry.kind} is not one of ${Object.keys(STATUSES).join(", ")}`);
      continue;
    }
    // An uncertifiable case stands for causes and expects results with them;
    // a violation case expects its violations; a clean case expects nothing.
    if (entry.kind === "uncertifiable" && (!Array.isArray(entry.causes) || entry.causes.length === 0)) found.push(`${where}: no causes`);
    if (entry.kind !== "clean" && (!Array.isArray(entry.expect) || entry.expect.length === 0)) found.push(`${where}: no expected result`);
    if (entry.kind === "clean" && (entry.expect ?? []).length) found.push(`${where}: a clean case expects no result`);
    for (const expected of entry.expect ?? []) {
      for (const field of entry.kind === "uncertifiable" ? ["rule", "family", "context", "cause"] : ["rule", "family", "context"]) {
        if (typeof expected[field] !== "string") found.push(`${where}: an expected result has no ${field}`);
      }
    }
    if (entry.kind === "uncertifiable") {
      // Results the classifier's label gives the case, the distinct source
      // sites among them (a file several projects analyze counts once), and
      // what a source read of a sample of those sites found.
      if (!Number.isInteger(entry.labelWeight) || entry.labelWeight < 0) found.push(`${where}: labelWeight is not a count`);
      if (!Number.isInteger(entry.sites) || entry.sites < 0 || entry.sites > entry.labelWeight) found.push(`${where}: sites is not a count of at most labelWeight`);
      const samples = entry.verification?.samples;
      if (!Array.isArray(samples)) found.push(`${where}: no verification samples`);
      for (const sample of samples ?? []) {
        if (!sample.app || !sample.path || !Number.isInteger(sample.line) || !sample.rule || !VERDICTS.includes(sample.verdict) || typeof sample.mechanism !== "string") {
          found.push(`${where}: a verification sample needs app, path, line, rule, a verdict (${VERDICTS.join(", ")}) and a mechanism`);
        }
      }
      if (entry.verifiedWeight !== verifiedWeight(entry)) found.push(`${where}: verifiedWeight ${entry.verifiedWeight} is not ${verifiedWeight(entry)} (its sites when every one of at least ${VERIFICATION_SAMPLE} samples reads the same, else 0)`);
    } else if (!Number.isInteger(entry.weight) || entry.weight < 0) found.push(`${where}: weight is not a count`);
    if (!STATUSES[entry.kind].includes(entry.status)) found.push(`${where}: status ${entry.status} is not one a ${entry.kind} case has (${STATUSES[entry.kind].join(", ")})`);
    const source = entry.source ?? {};
    if (!source.app || !source.path || !Number.isInteger(source.line) || !/^[0-9a-f]{40}$/.test(source.commit ?? "")) {
      found.push(`${where}: the corpus source needs app, path, line and a 40-hex commit`);
    } else if (manifest && manifest.apps?.[source.app]?.commit !== source.commit) {
      found.push(`${where}: ${source.app} is pinned at ${manifest.apps?.[source.app]?.commit ?? "nothing"} in the corpus manifest, not ${source.commit}`);
    }
  }
  for (const path of Object.keys(state.sources)) {
    if (!owner.has(path)) found.push(`${path}: belongs to no case in cases.json`);
  }
  const kindOf = new Map((state.table.cases ?? []).map((entry) => [entry.id, entry.kind]));
  for (const [kind, budget] of Object.entries(BUDGETS.kinds)) {
    const paths = Object.keys(state.sources).filter((path) => kindOf.get(owner.get(path)) === kind);
    const lines = paths.reduce((sum, path) => sum + state.sources[path], 0);
    if (paths.length > budget.files) found.push(`${kind} cases: ${paths.length} files, budget ${budget.files}`);
    if (lines > budget.lines) found.push(`${kind} cases: ${lines} lines, budget ${budget.lines}`);
  }

  // The shared node_modules holds exactly the published files the manifest
  // lists, byte for byte.
  const listed = new Map();
  for (const pkg of state.packages.packages ?? []) {
    if (!pkg.name || !pkg.version || !/^sha512-/.test(pkg.integrity ?? "")) found.push(`packages.json ${pkg.name ?? "(no name)"}: needs name, version and a sha512 integrity`);
    for (const [file, sha256] of Object.entries(pkg.files ?? {})) listed.set(`node_modules/${pkg.name}/${file}`, sha256);
  }
  let bytes = 0;
  for (const [path, { size, sha256 }] of Object.entries(state.modules)) {
    bytes += size;
    if (!listed.has(path)) found.push(`${path}: not listed in packages.json`);
    else if (listed.get(path) !== sha256) found.push(`${path}: sha256 ${sha256} is not the listed ${listed.get(path)}`);
  }
  for (const path of listed.keys()) if (!(path in state.modules)) found.push(`${path}: listed in packages.json but absent`);
  if (bytes > BUDGETS.nodeModulesBytes) found.push(`node_modules: ${bytes} bytes, budget ${BUDGETS.nodeModulesBytes}`);

  // `**/node_modules/` is ignored repository-wide; a file the exception lines
  // miss is present here and absent in CI (.claude/skills/add-fixture).
  if (tracked) {
    for (const path of [...Object.keys(state.modules), ...Object.keys(state.sources)]) {
      if (!tracked.has(`${SET}/${path}`)) found.push(`${SET}/${path}: not tracked by git`);
    }
  }
  return found;
}

export function trackedFiles() {
  return new Set(execFileSync("git", ["ls-files", "-z", "--", SET], { cwd: ROOT, encoding: "utf8" }).split("\0").filter(Boolean));
}

export function corpusManifest() {
  const path = join(ROOT, "scripts/corpus/rc17-manifest.json");
  return existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : null;
}

if (process.argv[1] && relative(process.argv[1], fileURLToPath(import.meta.url)) === "") {
  const found = violations(measure(), { tracked: trackedFiles(), manifest: corpusManifest() });
  for (const line of found) console.error(`repro-budget: ${line}`);
  if (found.length) process.exit(1);
  console.log("repro-budget: within budget");
}
