// The gate for AGENTS.md's absolute rule: a rule's positive case must not also
// be a `tsc` error.
//
// An absolute rule with no gate is a comment. This one takes every case in
// `fixtures/tsc-oracle/rule-cases.json` and asks *both* sides of the question
// about the same bytes: what TypeScript reports, by compiling against the
// *real published* Solid typings, and what this checker reports, by running it
// over the same snippet in its own project. So a new rule that duplicates a
// TypeScript diagnostic cannot be merged, a removal justified by a diagnostic
// cannot silently outlive it, and -- the half this gate was missing -- a
// narrowing cannot quietly turn a rule into a no-op.
//
//   node scripts/tsc-oracle-gate.mjs           enforce every case
//   node scripts/tsc-oracle-gate.mjs --json    machine-readable result
//   node scripts/tsc-oracle-gate.mjs --report  print both sides per case (for
//                                              writing a ledger entry, not for
//                                              gating)
//
// Each case therefore declares two things. `expect` is TypeScript's side and is
// documented in the ledger file itself; its escape hatch is `distinct-claim`,
// deliberately noisy, demanding the diagnostic codes *and* a written reason
// naming what the finding asserts that the type error does not. `checker` is
// this checker's side: `"reports"` or `"silent"`, enforced exactly.
//
// Two invariants tie the halves together:
//
//   `removed-because-redundant` requires `checker: "silent"`
//       The expectation's own words are "the checker has already stopped". That
//       was previously unverified, and a case could pin TypeScript's diagnostic
//       while the rule went on reporting the same expression.
//
//   every rule needs one `expect: "silent"` + `checker: "reports"` case
//       The keystone: a snippet where TypeScript says nothing and the rule still
//       speaks. It is this project's entire claim in one case, and the only
//       thing that makes a rule narrowed into silence fail rather than pass.
//
// There is no silent skip -- an unprovisioned oracle or a missing binary fails
// loudly, the same way `SOLID_TYPEFACTS_BIN`'s canary does.
//
// The 161 cases run concurrently, and the structure of this file is what keeps
// that honest. Execution -- two TypeScript programs and two checker processes
// per case -- happens in worker threads (`scripts/lib/tsc-oracle-case.mjs`),
// which is where the whole cost is. Every *verdict* is drawn here, in one pass
// over the cases in ledger order, so the failure list a run prints does not
// depend on which case finished first.
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { createWorkerPool, gateConcurrency, mapPool } from "./lib/pool.mjs";
import {
  canonicalRule,
  catalogEntries,
  prepareDialectBases,
} from "./lib/tsc-oracle-case.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const LEDGER = join(ROOT, "fixtures/tsc-oracle/rule-cases.json");
const WORKER = join(ROOT, "scripts/lib/tsc-oracle-gate-worker.mjs");

const json = process.argv.includes("--json");
const report = process.argv.includes("--report");
const ledger = JSON.parse(readFileSync(LEDGER, "utf8"));

const EXPECTATIONS = new Set(Object.keys(ledger.expectations));
const CASE_COMPILER_OPTIONS = new Set(["verbatimModuleSyntax"]);

/** Fail loudly rather than skip -- the same contract the oracle's provisioning check keeps. */
const locate = (variable, ...candidates) => {
  const found = process.env[variable] ?? candidates.find((candidate) => existsSync(candidate));
  if (!found || !existsSync(found)) {
    console.error(
      `missing the binary ${variable} names (tried ${candidates.join(", ")}).\n` +
        `This gate runs the checker over every case; without it only half of each` +
        ` case could be verified, and a half-checked gate is the thing this file exists to end.`,
    );
    process.exit(2);
  }
  return found;
};
const CHECKER = locate(
  "SOLID_CHECKER_BIN",
  join(ROOT, "rust/target/debug/solid-checker-rust"),
);
const TYPEFACTS = locate("SOLID_TYPEFACTS_BIN", join(ROOT, "bin/solid-typefacts"));

const subjectsOverlap = (finding, diagnostic) =>
  finding.subjectStartByte < diagnostic.subjectEndByte &&
  diagnostic.subjectStartByte < finding.subjectEndByte;

/**
 * Everything about a case that can be judged without running anything.
 *
 * `skip: true` reproduces the original loop's `continue`: a case whose shape is
 * wrong is not executed. The `presets`/`enableRules` check deliberately does
 * *not* skip -- its `continue` only ever left the inner field loop, so such a
 * case is still executed and still judged on both sides. Preserved as it was;
 * a case with a malformed `presets` should fail for that reason without also
 * losing its oracle verdict.
 */
const validate = (testCase, index) => {
  const label = `${testCase.rule} [${index}]`;
  const failures = [];
  const stop = (message) => {
    failures.push(message);
    return { failures, skip: true };
  };
  if (testCase.dialect !== "v1" && testCase.dialect !== "v2") {
    return stop(`${label}: dialect must be exactly "v1" or "v2"`);
  }
  if (testCase.dialect === "v2" && testCase.rule.startsWith("v1/")) {
    return stop(`${label}: a v2 case cannot name a v1/ catalog rule`);
  }
  if (testCase.sourceExtension !== undefined && !["ts", "tsx"].includes(testCase.sourceExtension)) {
    return stop(`${label}: sourceExtension must be exactly "ts" or "tsx"`);
  }
  if (testCase.compilerOptions !== undefined) {
    if (
      !testCase.compilerOptions ||
      Array.isArray(testCase.compilerOptions) ||
      typeof testCase.compilerOptions !== "object"
    ) {
      return stop(`${label}: compilerOptions must be an object`);
    }
    const unsupported = Object.keys(testCase.compilerOptions).filter(
      (name) => !CASE_COMPILER_OPTIONS.has(name),
    );
    if (unsupported.length) {
      return stop(`${label}: unsupported compilerOptions ${unsupported.join(", ")}`);
    }
    if (
      testCase.compilerOptions.verbatimModuleSyntax !== undefined &&
      typeof testCase.compilerOptions.verbatimModuleSyntax !== "boolean"
    ) {
      return stop(`${label}: compilerOptions.verbatimModuleSyntax must be boolean`);
    }
  }
  for (const field of ["presets", "enableRules"]) {
    if (
      testCase[field] !== undefined &&
      (!Array.isArray(testCase[field]) || testCase[field].some((value) => typeof value !== "string"))
    ) {
      failures.push(`${label}: ${field} must be an array of strings`);
    }
  }
  if (!EXPECTATIONS.has(testCase.expect)) {
    return stop(`${label}: unknown expectation ${JSON.stringify(testCase.expect)}`);
  }
  if (!testCase.why || testCase.why.length < 20) {
    return stop(`${label}: every case needs a written 'why'`);
  }
  return { failures, skip: false };
};

/**
 * Judge one executed case: both declarations, the duplicate-subject audit, and
 * TypeScript's own expectation.
 *
 * Pure -- it reads only the case and what the worker observed -- so the order
 * of this file's failure list is the ledger's order and nothing else.
 */
const evaluate = (testCase, index, { perPass, checkerPasses }) => {
  const label = `${testCase.rule} [${index}]`;
  const failures = [];
  const expectedRule = canonicalRule(testCase);
  const seen = [...new Set(perPass.flatMap(([, diagnostics]) => diagnostics.map((d) => d.code)))].sort(
    (a, b) => a - b,
  );
  const targetByPass = checkerPasses.map(([name, observed]) => [
    name,
    observed.findings.filter((finding) => finding.rule === expectedRule),
  ]);
  const observedRules = [
    ...new Set(checkerPasses.flatMap(([, observed]) => observed.findings.map((finding) => finding.rule))),
  ].sort();
  const result = {
    rule: testCase.rule,
    index,
    expect: testCase.expect,
    checker: testCase.checker,
    observedChecker: Object.fromEntries(
      targetByPass.map(([name, findings]) => [name, findings.length ? "reports" : "silent"]),
    ),
    observedRules,
    checkerPasses,
    codes: seen,
    perPass,
  };
  const done = () => ({ result, failures });

  if (testCase.checker !== "reports" && testCase.checker !== "silent") {
    failures.push(
      `${label}: every case must declare 'checker' as "reports" or "silent" --` +
        ` what this checker says about these bytes, next to what TypeScript says.` +
        ` The oracle ran both strict and loose checker projects.`,
    );
  } else {
    for (const [name, findings] of targetByPass) {
      const reports = findings.length > 0;
      if (reports !== (testCase.checker === "reports")) {
        failures.push(
          testCase.checker === "reports"
            ? `${label}: declared 'checker: "reports"' but ${expectedRule} is silent in the ${name} project.` +
                `\n    Findings seen: ${observedRules.length ? observedRules.join(", ") : "none"}.`
            : `${label}: declared 'checker: "silent"' but ${expectedRule} fired in the ${name} project.`,
        );
      }
      if (reports && findings.some((finding) => finding.kind !== (testCase.checkerKind ?? "violation"))) {
        failures.push(
          `${label}: ${name} emitted the wrong finding kind for ${expectedRule}; expected ` +
            `${testCase.checkerKind ?? "violation"}, saw ${[
              ...new Set(findings.map((finding) => finding.kind)),
            ].join(", ")}.`,
        );
      }
      const expectedCount = testCase.checker === "reports" ? (testCase.checkerCount ?? 1) : 0;
      if (findings.length !== expectedCount) {
        failures.push(
          `${label}: ${name} emitted ${findings.length} ${expectedRule} finding(s), expected ` +
            `${expectedCount}. Declare checkerCount only when one isolated case intentionally contains multiple subjects.`,
        );
      }
    }
  }

  // Audit every finding on a TypeScript-owned source subject. Checking only
  // the case's named rule lets another rule duplicate the same diagnostic and
  // still pass. A distinct claim must be explicit at this case, not inferred
  // from the rule having been legitimate somewhere else.
  const distinctFindings = new Map(
    (testCase.distinctFindings ?? []).map((entry) => [
      testCase.dialect === "v1" && !entry.rule.startsWith("v1/") ? `v1/${entry.rule}` : entry.rule,
      entry.why,
    ]),
  );
  for (const [passName, diagnostics] of perPass) {
    const checker = checkerPasses.find(([name]) => name === passName)[1];
    for (const finding of checker.findings) {
      const overlapping = diagnostics.filter((diagnostic) => subjectsOverlap(finding, diagnostic));
      if (!overlapping.length) continue;
      const targetIsDistinct = finding.rule === expectedRule && testCase.expect === "distinct-claim";
      const writtenDistinct = distinctFindings.get(finding.rule);
      if (targetIsDistinct || (writtenDistinct && writtenDistinct.length >= 20)) continue;
      failures.push(
        `${label}: ${passName} checker finding ${finding.rule} (${finding.kind}) shares source subject ` +
          `with ${overlapping.map((d) => `TS${d.code}`).join("/")}.` +
          `\n    Remove/narrow the duplicate, make the case isolate its claim, or add a` +
          ` case-local distinctFindings entry with the different semantic assertion.`,
      );
    }
  }

  if (testCase.expect === "silent") {
    for (const [name, diagnostics] of perPass) {
      if (diagnostics.length) {
        failures.push(
          `${label}: expected silence but the ${name} pass reported ` +
            diagnostics.map((d) => `TS${d.code} at ${d.line}:${d.column} (${d.message})`).join("; ") +
            `\n    This rule now duplicates a TypeScript diagnostic. Narrow it to the uncovered` +
            ` spellings or delete it -- see AGENTS.md's absolute rule.`,
        );
      }
    }
    return done();
  }

  // The remaining expectations all require a diagnostic, and require it to be
  // one the ledger already accounted for. A *new* code means the case changed
  // meaning, which has to be re-explained rather than absorbed.
  const allow = new Set(testCase.allow ?? []);
  if (!allow.size) {
    failures.push(`${label}: ${testCase.expect} requires 'allow' listing the diagnostic codes`);
    return done();
  }
  if (!seen.length) {
    failures.push(
      `${label}: expected TypeScript to report ${[...allow].map((c) => `TS${c}`).join("/")} but it was silent.` +
        {
          "removed-because-redundant":
            `\n    The removal of this rule was justified by that diagnostic. It is gone, so the` +
            ` defect is now unreported by anyone -- reconsider the removal.`,
          "redundant-pending-narrowing":
            `\n    This case was recorded as a duplicate awaiting narrowing. TypeScript no longer` +
            ` speaks, so the duplicate is gone and the debt entry should be closed.`,
          "distinct-claim": `\n    The 'distinct-claim' allowance no longer describes anything.`,
        }[testCase.expect],
    );
    return done();
  }
  const unexpected = seen.filter((code) => !allow.has(code));
  if (unexpected.length) {
    failures.push(
      `${label}: unexpected ${unexpected.map((c) => `TS${c}`).join("/")} (declared: ` +
        `${[...allow].map((c) => `TS${c}`).join("/")}); re-explain the case`,
    );
  }
  return done();
};

// Phase 1: shape. A case that cannot be read is not executed, exactly as
// before -- and an unprovisioned oracle fails here, once, rather than as N
// identical worker errors.
const validated = ledger.cases.map((testCase, index) => validate(testCase, index));
const runnable = ledger.cases
  .map((testCase, index) => ({ testCase, index }))
  .filter(({ index }) => !validated[index].skip);

prepareDialectBases();

// Phase 2: execution, concurrent and order-independent.
const concurrency = gateConcurrency();
const pool = createWorkerPool({
  workerPath: WORKER,
  size: Math.min(concurrency, Math.max(1, runnable.length)),
  workerData: { checker: CHECKER, typefacts: TYPEFACTS },
});
const executed = new Map();
try {
  const observed = await mapPool(runnable, ({ testCase, index }) => pool.run({ testCase, index }), {
    concurrency,
  });
  for (const [position, { index }] of runnable.entries()) executed.set(index, observed[position]);
} finally {
  await pool.close();
}

// Phase 3: judgement, in ledger order.
const failures = [];
const results = [];
for (const [index, testCase] of ledger.cases.entries()) {
  failures.push(...validated[index].failures);
  if (validated[index].skip) continue;
  const judged = evaluate(testCase, index, executed.get(index));
  results.push(judged.result);
  failures.push(...judged.failures);
}

// The mechanism that makes the rule permanent rather than documented: a catalog
// rule with no case here cannot be judged, so the gate refuses to let one exist
// silently. A new rule must arrive with its positive spelling and the oracle's
// verdict on it.
const EXEMPT = {
  "package-contract-incomplete": "asks whether a package ships a usable reactivity contract, which is an analyzability fact about an external artifact; no snippet against real Solid typings can express it",
  "v1/package-contract-incomplete": "same -- the subject is a third-party package's contract, not Solid's types",
  "server-function-module-directive": "needs a module-level \"use server\" prologue and the project's server surface",
};

const catalogRules = catalogEntries.map((rule) => rule.name);

const declared = new Set(ledger.cases.map(canonicalRule));
const uncovered = [...new Set(catalogRules)].filter(
  (name) => !declared.has(name) && !(name in EXEMPT),
);
// `removed-because-redundant` asserts the checker has already stopped. That is
// half the claim, and it was the unverifiable half: a case could pin the
// TypeScript diagnostic while the rule quietly went on reporting the same
// expression. Now it cannot.
for (const [index, testCase] of ledger.cases.entries()) {
  if (testCase.expect === "removed-because-redundant" && testCase.checker === "reports") {
    failures.push(
      `${testCase.rule} [${index}]: expectation "removed-because-redundant" means the checker has` +
        ` stopped reporting here, but it declares 'checker: "reports"'.` +
        `\n    If the surviving claim is genuinely a different one, this case is a` +
        ` 'distinct-claim'. If it is the same claim, the narrowing is incomplete. If the case` +
        ` merely bundles a still-reported spelling alongside the removed one, split them --` +
        ` the removed arm should be shown on code the checker is silent about.`,
    );
  }
  // The mirror image: `distinct-claim` keeps a finding *because* it says
  // something the type error does not, so there has to be a finding.
  if (testCase.expect === "distinct-claim" && testCase.checker === "silent") {
    failures.push(
      `${testCase.rule} [${index}]: expectation "distinct-claim" means the checker reports here and` +
        ` means something other than the type error, but it declares 'checker: "silent"'.` +
        `\n    There is no finding left to be distinct from. If the rule has stopped reporting,` +
        ` this case is "removed-because-redundant".`,
    );
  }
  if (testCase.expect === "redundant-pending-narrowing" && testCase.checker !== "reports") {
    failures.push(
      `${testCase.rule} [${index}]: redundant-pending-narrowing must still report until the` +
        ` semantic narrowing lands; use removed-because-redundant only after it is silent.`,
    );
  }
}

// Aliveness: no rule may be silent everywhere.
//
// Per-case declarations pin what each case *is*; this pins that something must
// be. Without it a narrowing can walk a rule all the way to a no-op and every
// case still passes -- each one truthfully declaring `checker: "silent"`, the
// gate satisfied, the rule dead.
//
// A rule proves it is alive with a keystone case where TypeScript is silent and
// the rule speaks. Coverage snapshots have their own fresh-binary gate; reading
// their checked-in state here made this target order-dependent and let a stale
// snapshot substitute for an executable oracle case.
const keystoneByRule = new Map();
for (const testCase of ledger.cases) {
  const name = canonicalRule(testCase);
  keystoneByRule.set(
    name,
    keystoneByRule.get(name) || (testCase.expect === "silent" && testCase.checker === "reports"),
  );
}
// Only rules the catalog still ships. A case for a *removed* rule is kept as a
// regression record -- "the removal took nothing with it", pinned so a future
// release that loosens a type cannot quietly reopen the hole -- and there is no
// rule left for it to keep alive.
const shipped = new Set(catalogRules);
for (const [name, has] of keystoneByRule) {
  if (!shipped.has(name)) continue;
  if (has || name in EXEMPT) continue;
  failures.push(
    `${name}: nothing shows this rule still reports anywhere.` +
      `\n    No exact-dialect case pairs 'expect: "silent"' with 'checker: "reports"'.` +
      ` A rule that reports nowhere is dead or untested. Give it a keystone case, or an EXEMPT` +
      ` entry saying why no snippet can express its subject.`,
  );
}

for (const name of uncovered) {
  failures.push(
    `${name}: no case in fixtures/tsc-oracle/rule-cases.json.` +
      `\n    Every catalog rule needs its positive spelling here so the oracle can answer` +
      ` "does TypeScript already report this?". Add a case, or an EXEMPT entry in this` +
      ` script saying why no snippet can express the rule's subject.`,
  );
}

if (report) {
  for (const result of results) {
    console.log(
      `\n=== ${result.rule} [${result.index}] expect=${result.expect}` +
        ` checker=${result.checker ?? "<undeclared>"} (observed ` +
        `${JSON.stringify(result.observedChecker)})`,
    );
    console.log(`  findings: ${result.observedRules.length ? result.observedRules.join(", ") : "none"}`);
    for (const [name, diagnostics] of result.perPass) {
      console.log(`  ${name}: ${diagnostics.length || "clean"}`);
      for (const d of diagnostics) {
        console.log(`    TS${d.code} ${d.line}:${d.column} [${d.startByte}..${d.endByte}] ${d.message}`);
      }
    }
  }
}

if (json) {
  console.log(
    JSON.stringify(
      { cases: results.map(({ perPass, checkerPasses, ...rest }) => rest), failures },
      null,
      2,
    ),
  );
} else if (failures.length) {
  console.error(`tsc oracle gate: ${failures.length} case(s) failed\n`);
  for (const failure of failures) console.error(`  - ${failure}\n`);
} else {
  const keystones = [...keystoneByRule.values()].filter(Boolean).length;
  console.log(
    `tsc oracle gate: ${ledger.cases.length} case(s) hold on both sides` +
      ` (TypeScript and the checker); ${keystones} rule(s) carry a silent-tsc/reporting-rule keystone` +
      ` [concurrency ${concurrency}]`,
  );
}

process.exit(failures.length ? 1 : 0);
