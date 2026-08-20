// Runs every eslint-plugin-solid test case against the checker's v1 rules.
//
// The corpus is `fixtures/upstream-parity/upstream-cases.json` — 465 cases
// extracted from upstream's own suites (see the README there). Each case is
// materialised as its own file in one synthetic project, the checker runs
// once, and three things are compared against what upstream's own test
// asserts:
//
//   fired    the rule under test spoke for an `invalid` case and stayed
//            silent for a `valid` one
//   counts   how many diagnostics it produced, against upstream's `errors`
//   outputs  what applying its fixes to the case source produces, against
//            upstream's `output`
//
// Counts and outputs are only asked of a case that already agrees on
// `fired` — a case where the rule stayed silent has no count or fix to
// compare, and recording 0-against-2 would be the same disagreement twice.
//
// Deviations are declared in `deviations.json`, one ledger per dimension,
// one entry per case with a status and a reason. The comparison is exact in
// every direction: a case that starts deviating fails, one that stops fails,
// and one whose numbers or fix text merely *change* fails too, so an
// inherited false positive cannot arrive quietly and a fix cannot silently
// rot.
//
//   node scripts/parity.mjs             compare against the declared deviations
//   node scripts/parity.mjs --update    rewrite deviations.json from this run
//
// `--update` keeps existing reasons and marks anything new `triage`, which
// the comparison rejects — a new deviation has to be explained, not recorded.
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { CORPUS, caseId, corpusCases, materialize } from "./lib/upstream-cases.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const update = process.argv.includes("--update");
const corpus = corpusCases();

// The three dimensions the comparison ratchets, each its own ledger in
// `deviations.json` with its own status vocabulary (the README's tables
// document every status). A retired status is removed from these sets so it
// cannot quietly come back — `unsupported-option` was one, emptied when the
// option-bearing rules grew upstream's options surface.
const LEDGERS = ["fired", "counts", "outputs"];
// What each ledger records about the deviation besides its status and
// reason. These are compared too, so a deviation that merely changes
// magnitude — 2-against-1 becoming 3-against-1 — has to be re-explained
// instead of coasting on the declaration it outgrew.
const VALUE_KEYS = { fired: [], counts: ["ours", "theirs"], outputs: ["ours"] };
const ALLOWED_STATUSES = {
  // Whether the rule spoke at all. `typescript-owned` is `policy`'s narrower
  // sibling and exists so the claim can be *checked*: it asserts that the defect
  // is already a TypeScript diagnostic on this exact code, which
  // `scripts/parity-tsc-ownership.mjs` verifies against the real published
  // typings. Plain `policy` stays for the deliberate differences that make no
  // such claim -- a name-only allowlist this checker does not carry, a stricter
  // demand for an explicit `untrack` -- and those are unverifiable by
  // construction, which is exactly why they must not share a status with the
  // ones that are.
  fired: new Set(["evidence-backed", "fact-unavailable", "policy", "typescript-owned"]),
  // How many times it spoke.
  counts: new Set(["per-site", "rule-split"]),
  // What its fixes produce.
  outputs: new Set(["tighter-cleanup", "different-strategy", "cosmetic"]),
};
const declared = existsSync(join(CORPUS, "deviations.json"))
  ? JSON.parse(readFileSync(join(CORPUS, "deviations.json"), "utf8"))
  : {};
for (const ledger of LEDGERS) declared[ledger] ??= {};

// Upstream's `reactivity` is one rule behind eight message ids; the checker
// splits it. Mirrors the table in docs/rules/README.md.
const REACTIVITY = {
  untrackedReactive: ["v1/strict-read-untracked", "v1/reactive-read-after-await", "v1/no-destructure", "v1/untracked-derived-function"],
  badSignal: ["v1/uncalled-accessor"],
  // A runWithOwner callback is a direct untracked read in the checker (the
  // runtime clears Listener); upstream calls the same defect an unnamed
  // derived signal. Either evidence-backed diagnosis satisfies that case.
  badUnnamedDerivedSignal: ["v1/untracked-derived-function", "v1/strict-read-untracked"],
  // Passing a reactive prop member where a callback is expected is also an
  // untracked read of that member. The checker may surface the more general
  // proof when the callback's callable type itself is unresolved.
  expectedFunctionGotExpression: ["v1/reactive-handler-frozen", "v1/strict-read-untracked"],
  noWrite: ["v1/no-direct-mutation"],
  noAsyncTrackedScope: ["v1/no-async-tracked-scope"],
  // Upstream's analysis-integrity warnings (a createSignal/createMemo result
  // not captured in a shape its analyzer can follow) have no v1 rule: the
  // checker resolves sources through TypeScript symbols and is not blinded by
  // capture shape. Their invalid cases are evidence-backed deviations.
  shouldDestructure: [],
  shouldAssign: [],
};
const ALL_REACTIVITY = [...new Set(Object.values(REACTIVITY).flat())];

// The rules whose upstream options the checker carries in
// `.solid-checker/rule-options.json`. A configured case for one of these
// runs in its own project with the case's options written to that file;
// configured cases for any other rule run with defaults and stay declared
// deviations.
const CONFIGURABLE = new Set([
  "event-handlers",
  "no-innerhtml",
  "self-closing-comp",
  "prefer-classlist",
  "style-prop",
  "no-unknown-namespaces",
]);

const harness = JSON.parse(readFileSync(join(CORPUS, "harness.json"), "utf8"));
const materializeProject = (directory) => {
  rmSync(directory, { recursive: true, force: true });
  mkdirSync(directory, { recursive: true });
  for (const [path, body] of Object.entries(harness)) {
    mkdirSync(join(directory, dirname(path)), { recursive: true });
    writeFileSync(join(directory, path), body);
  }
};
const writeCase = (directory, id, materialized) =>
  writeFileSync(join(directory, `${id}.tsx`), materialized.source);
const locate = (variable, ...candidates) => {
  const override = process.env[variable];
  if (override) return override;
  return candidates.find((candidate) => existsSync(candidate)) ?? candidates[0];
};
const CHECKER = locate(
  "SOLID_CHECKER_BIN",
  join(ROOT, "bin/solid-checker-rust"),
  join(ROOT, "rust/target/debug/solid-checker-rust"),
);
const TYPEFACTS = locate("SOLID_TYPEFACTS_BIN", join(ROOT, "bin/solid-typefacts"));
for (const [name, path] of [
  ["checker", CHECKER],
  ["type facts producer", TYPEFACTS],
]) {
  if (!existsSync(path)) {
    console.error(`missing ${name} at ${path} -- run 'make build-rust' first`);
    process.exit(2);
  }
}

const runChecker = (directory) =>
  execFileSync(CHECKER, [
    "--format", "json", "--project", join(directory, "tsconfig.json"),
  ], {
    env: { ...process.env, SOLID_TYPEFACTS_BIN: TYPEFACTS },
    encoding: "utf8",
    maxBuffer: 1 << 28,
  });

const project = join(ROOT, "rust/target/upstream-parity");
materializeProject(project);

// Configured cases grouped by (rule, exact options), one project per group;
// everything else shares the default-options project.
const cases = [];
const configured = new Map();
for (const rule of corpus) {
  for (const kind of ["valid", "invalid"]) {
    rule[kind].forEach((testCase, index) => {
      const id = caseId(rule.rule, kind, index);
      const options = testCase.options?.[0];
      const materialized = materialize(rule.rule, testCase);
      if (options && CONFIGURABLE.has(rule.rule)) {
        const key = `${rule.rule}::${JSON.stringify(options)}`;
        if (!configured.has(key)) configured.set(key, { rule: rule.rule, options, members: [] });
        configured.get(key).members.push({ id, materialized });
      } else {
        writeCase(project, id, materialized);
      }
      cases.push({ id, rule: rule.rule, kind, ...testCase, materialized });
    });
  }
}

const runs = [runChecker(project)];
let group = 0;
for (const { rule, options, members } of configured.values()) {
  const directory = join(ROOT, `rust/target/upstream-parity-options-${group++}`);
  materializeProject(directory);
  mkdirSync(join(directory, ".solid-checker"), { recursive: true });
  writeFileSync(
    join(directory, ".solid-checker/rule-options.json"),
    `${JSON.stringify({ schemaVersion: 1, rules: { [`v1/${rule}`]: options } }, null, 2)}\n`,
  );
  for (const { id, materialized } of members) writeCase(directory, id, materialized);
  runs.push(runChecker(directory));
}

const found = new Map();
for (const output of runs) {
  for (const finding of JSON.parse(output).findings ?? []) {
    // Separator-agnostic: the checker reports whichever separator the host
    // OS uses, and a Windows path never splits on "/".
    const id = finding.primaryLocation.path.split(/[\\/]/).pop().replace(/\.tsx$/, "");
    if (!found.has(id)) found.set(id, []);
    found.get(id).push(finding);
  }
}

// The run artifact: which findings landed where, keyed by case id, with the byte
// spans the checker reported. `scripts/parity-tsc-ownership.mjs` compiles the
// *same bytes* against the real published typings and needs these spans to ask
// whether a TypeScript diagnostic covers the same expression -- co-presence in a
// file proves nothing on an untyped corpus, where most cases carry incidental
// implicit-any errors. Written to the build directory, not committed: it is a
// product of this run and stale spans would be worse than none.
writeFileSync(
  join(project, "findings.json"),
  `${JSON.stringify(
    {
      schemaVersion: 1,
      cases: Object.fromEntries(
        [...found].map(([id, findings]) => [
          id,
          findings.map((finding) => ({
            rule: finding.rule,
            code: finding.id,
            startByte: finding.primaryLocation.startByte,
            endByte: finding.primaryLocation.endByte,
          })),
        ]),
      ),
    },
    null,
    2,
  )}\n`,
);

// Applies one pass of `fixes` to `source`, the way ESLint's own fixer does —
// which is what makes the result comparable to a RuleTester `output`, itself
// the product of exactly one pass. Edits go in source order, and a fix whose
// edits overlap text an earlier fix already rewrote is skipped, as ESLint
// skips it (upstream's own `output` records such a partially-fixed state for
// `imports__invalid__05`). A fix is all-or-nothing: applying half of a
// multi-edit fix would emit text no fix ever proposed. Byte offsets, so a
// non-ASCII case cannot slip.
const applyFixes = (source, fixes) => {
  const bytes = Buffer.from(source, "utf8");
  const ordered = fixes
    .map((fix) => [...fix.edits].sort((a, b) => a.location.startByte - b.location.startByte))
    .sort((a, b) => a[0].location.startByte - b[0].location.startByte);
  const pieces = [];
  let cursor = 0;
  for (const edits of ordered) {
    if (edits[0].location.startByte < cursor) continue;
    for (const edit of edits) {
      pieces.push(bytes.subarray(cursor, edit.location.startByte), Buffer.from(edit.newText, "utf8"));
      cursor = edit.location.endByte;
    }
  }
  pieces.push(bytes.subarray(cursor));
  return Buffer.concat(pieces).toString("utf8");
};

// Undoes the harness's own additions so what remains is the case text
// upstream's `output` describes. Nothing else is touched: no whitespace is
// normalised, no trailing newline is forgiven beyond the single one the
// harness itself appended. `null` when a fix rewrote across an affix, which
// leaves nothing sound to compare.
const withoutAffixes = (text, { prefix, suffix }) =>
  text.startsWith(prefix) && text.endsWith(suffix) && text.length >= prefix.length + suffix.length
    ? text.slice(prefix.length, text.length - suffix.length)
    : null;

// The three ledgers, keyed by case id. `fired` is whether the rule spoke at
// all, `counts` how many times, `outputs` what its fixes produce.
const observed = { fired: {}, counts: {}, outputs: {} };
// Cases the harness could not compare on a dimension at all, reported in the
// summary rather than ratcheted: an absent fix is a coverage gap, not a
// disagreement about what the right fix is.
const uncompared = { noFix: [], ambiguousFix: [], affixCrossed: [] };
const totals = { counts: 0, outputs: 0 };
let matched = 0;

// Keeps a declared entry's reason only while it still describes this run:
// when the observed values move, the entry goes back to `triage` with its
// old reason visible, because a reason written for 2-vs-1 does not explain
// 3-vs-1.
const carry = (previous, values) => {
  const stale =
    !previous || Object.entries(values).some(([key, value]) => previous[key] !== value);
  return { ...values, status: stale ? "triage" : previous.status, reason: previous?.reason ?? "" };
};

for (const testCase of cases) {
  const findings = found.get(testCase.id) ?? [];
  const relevant =
    testCase.rule === "reactivity" ? ALL_REACTIVITY : [`v1/${testCase.rule}`];
  const expected =
    testCase.rule === "reactivity" && testCase.messageIds.length
      ? [...new Set(testCase.messageIds.flatMap((id) => REACTIVITY[id] ?? []))]
      : relevant;
  const wanted = testCase.kind === "invalid";
  const got = findings.some((finding) => (wanted ? expected : relevant).includes(finding.rule));
  if (got !== wanted) {
    observed.fired[testCase.id] = declared.fired[testCase.id] ?? { status: "triage", reason: "" };
    // A case that does not agree on whether the rule fires has nothing to
    // say about how many times or with what fix; comparing 0 findings
    // against upstream's 2 would record the same disagreement twice.
    continue;
  }
  matched++;
  if (!wanted) continue;

  // Dimension two: how many diagnostics. Upstream's `reactivity` is one
  // rule, so every checker rule it maps onto counts towards the same total.
  const mine = findings.filter((finding) => relevant.includes(finding.rule));
  totals.counts++;
  if (mine.length !== testCase.errors) {
    observed.counts[testCase.id] = carry(declared.counts[testCase.id], {
      ours: mine.length,
      theirs: testCase.errors,
    });
  }

  // Dimension three: what the fixes produce. Only comparable when upstream
  // declares an autofix result and the checker offers a fix to apply.
  if (!testCase.output) continue;
  const fixes = mine.flatMap((finding) => finding.fixes ?? []);
  if (!fixes.length) {
    uncompared.noFix.push(testCase.id);
    continue;
  }
  if (mine.some((finding) => (finding.fixes ?? []).length > 1)) {
    // Several fixes on one finding are alternatives to choose between, and
    // the harness has no basis for choosing; applying them all would emit
    // text no single fix proposed.
    uncompared.ambiguousFix.push(testCase.id);
    continue;
  }
  const ours = withoutAffixes(
    applyFixes(testCase.materialized.source, fixes),
    testCase.materialized,
  );
  if (ours === null) {
    uncompared.affixCrossed.push(testCase.id);
    continue;
  }
  totals.outputs++;
  if (ours !== testCase.output) {
    observed.outputs[testCase.id] = carry(declared.outputs[testCase.id], { ours });
  }
}

const invalid = cases.filter((testCase) => testCase.kind === "invalid").length;
const counted = (ledger) => Object.keys(observed[ledger]).length;
console.log(
  `${matched}/${cases.length} upstream cases match; ${counted("fired")} deviate\n` +
    `${totals.counts}/${invalid} invalid cases compared on diagnostic count; ${counted("counts")} deviate\n` +
    `${totals.outputs} of those compared on fix output; ${counted("outputs")} deviate` +
    ` (${uncompared.noFix.length} upstream autofixes the checker offers no fix for)`,
);
for (const [what, ids] of [
  ["a finding offering several alternative fixes", uncompared.ambiguousFix],
  ["a fix rewriting across the harness's own affixes", uncompared.affixCrossed],
]) {
  if (ids.length) console.log(`not compared on fix output, ${what}: ${ids.join(", ")}`);
}

const sortEntries = (entries) =>
  Object.fromEntries(Object.entries(entries).sort(([a], [b]) => a.localeCompare(b)));

if (update) {
  const next = {
    schemaVersion: 2,
    fired: sortEntries(observed.fired),
    counts: sortEntries(observed.counts),
    outputs: sortEntries(observed.outputs),
  };
  writeFileSync(join(CORPUS, "deviations.json"), `${JSON.stringify(next, null, 2)}\n`);
  const all = LEDGERS.flatMap((ledger) => Object.values(next[ledger]));
  const triage = all.filter((entry) => entry.status === "triage").length;
  console.log(`recorded ${all.length} deviations${triage ? `, ${triage} still to triage` : ""}`);
  process.exit(0);
}

let failed = false;
for (const ledger of LEDGERS) {
  const mine = observed[ledger];
  const theirs = declared[ledger] ?? {};
  for (const id of Object.keys(mine).filter((id) => !(id in theirs))) {
    console.error(`new ${ledger} deviation, not declared: ${id}`);
    failed = true;
  }
  for (const id of Object.keys(theirs).filter((id) => !(id in mine))) {
    console.error(`declared ${ledger} deviation no longer deviates, remove it: ${id}`);
    failed = true;
  }
  for (const [id, entry] of Object.entries(mine)) {
    if (!(id in theirs)) continue;
    // A declaration that no longer describes what this run observed: the
    // deviation is still there but has changed shape, and a reason written
    // for 2-against-1 does not explain 3-against-1. Reported before the
    // status check, which `carry` has already demoted to `triage`.
    const moved = VALUE_KEYS[ledger].filter((key) => entry[key] !== theirs[id][key]);
    if (moved.length) {
      const changes = moved
        .map((key) => `${key} ${JSON.stringify(theirs[id][key])} -> ${JSON.stringify(entry[key])}`)
        .join(", ");
      console.error(`declared ${ledger} deviation no longer describes this run (${changes}): ${id}`);
      failed = true;
      continue;
    }
    if (!ALLOWED_STATUSES[ledger].has(entry.status) || !entry.reason) {
      console.error(`${ledger} deviation has an invalid status or no reason: ${id}`);
      failed = true;
    }
  }
}
if (failed) {
  console.error("\nre-run with --update once every deviation above is triaged");
  process.exit(1);
}
const summary = LEDGERS.map((ledger) => {
  const byStatus = {};
  for (const entry of Object.values(declared[ledger])) {
    byStatus[entry.status] = (byStatus[entry.status] ?? 0) + 1;
  }
  const counts = Object.entries(byStatus).sort().map(([status, n]) => `${n} ${status}`);
  return `${ledger} ${counts.length ? counts.join("/") : "none"}`;
}).join("; ");
const all = LEDGERS.reduce((sum, ledger) => sum + Object.keys(declared[ledger]).length, 0);
console.log(`all ${all} deviations declared: ${summary}`);
