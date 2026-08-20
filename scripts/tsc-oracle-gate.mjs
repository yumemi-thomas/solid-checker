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
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import {
  oracleCompilerOptions,
  oracleProject,
  oracleSubjectSpan,
  runOracle,
} from "./tsc-oracle.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const LEDGER = join(ROOT, "fixtures/tsc-oracle/rule-cases.json");
// Case projects live under the one ignored build root, beside the audited
// installs they resolve against.
const CASE_ROOT = join(ROOT, "rust/target/tsc-oracle-cases");

const json = process.argv.includes("--json");
const report = process.argv.includes("--report");
const ledger = JSON.parse(readFileSync(LEDGER, "utf8"));

const EXPECTATIONS = new Set(Object.keys(ledger.expectations));
const CASE_COMPILER_OPTIONS = new Set(["verbatimModuleSyntax"]);

// Both passes matter. "Only under `strict`" is not an exception the absolute
// rule recognises, so a case is redundant if *either* pass reports it; a case
// claiming silence has to be silent in both.
const passes = (result) => [
  ["strict", result.passes.strict],
  ["loose", result.passes.loose],
];

const errorsOnly = (diagnostics) => diagnostics.filter((d) => d.category === "error");

const canonicalRule = (testCase) =>
  testCase.dialect === "v1" && !testCase.rule.startsWith("v1/")
    ? `v1/${testCase.rule}`
    : testCase.rule;
const slug = (name) => name.replace(/[^a-z0-9]+/gi, "-");
const sourceName = (testCase, index) =>
  `${slug(testCase.rule)}-${index}.${testCase.sourceExtension ?? "tsx"}`;

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

// One directory per dialect, holding a symlink to that dialect's audited
// install. A symlink rather than a copy because the checker picks its dialect
// from the nearest `node_modules/solid-js` above the project -- so the tree the
// oracle compiles against is also the tree that decides which catalog runs --
// and because the audited install must stay read-only.
const prepared = new Map();
const dialectBase = (dialect) => {
  if (prepared.has(dialect)) return prepared.get(dialect);
  const { root } = oracleProject(dialect);
  const base = join(CASE_ROOT, dialect);
  mkdirSync(base, { recursive: true });
  const link = join(base, "node_modules");
  if (!existsSync(link)) symlinkSync(join(root, "node_modules"), link, "dir");
  const entry = { base };
  prepared.set(dialect, entry);
  return entry;
};

/**
 * Run the checker over one case, in its own project.
 *
 * Its own project, not one program over all of them: a case is a claim about
 * what the checker says on exactly these bytes, and project-level analysis --
 * source discovery, owner reachability, contract lookups -- can differ once
 * unrelated files join. The compiler options mirror the oracle's `strict` pass
 * so both halves of a case describe the same program.
 */
const runChecker = (testCase, index, strict) => {
  const { base } = dialectBase(testCase.dialect);
  const dir = join(base, `case-${index}`);
  mkdirSync(dir, { recursive: true });
  const pass = strict ? "strict" : "loose";
  const caseSourceName = sourceName(testCase, index);
  const code = testCase.code.endsWith("\n") ? testCase.code : `${testCase.code}\n`;
  writeFileSync(
    join(dir, `tsconfig.${pass}.json`),
    `${JSON.stringify(
      {
        compilerOptions: oracleCompilerOptions(
          testCase.dialect,
          strict,
          testCase.compilerOptions,
        ),
        files: [caseSourceName],
      },
      null,
      2,
    )}\n`,
  );
  writeFileSync(
    join(dir, caseSourceName),
    code,
  );
  const enablementArgs = [
    ...(testCase.presets ?? []).flatMap((preset) => ["--preset", preset]),
    ...(testCase.enableRules ?? []).flatMap((rule) => ["--enable-rule", rule]),
  ];
  const output = execFileSync(
    CHECKER,
    ["--format", "json", "--project", join(dir, `tsconfig.${pass}.json`), ...enablementArgs],
    {
      encoding: "utf8",
      maxBuffer: 256 * 1024 * 1024,
      env: { ...process.env, SOLID_TYPEFACTS_BIN: TYPEFACTS },
    },
  );
  const snapshot = JSON.parse(output);
  const findings = (snapshot.findings ?? []).map((finding) => {
    const location = finding.primaryLocation;
    const subject = oracleSubjectSpan(code, location.startByte, location.endByte, caseSourceName);
    return {
      id: finding.id,
      rule: finding.rule,
      kind: finding.kind,
      startByte: location.startByte,
      endByte: location.endByte,
      subjectStartByte: subject.startByte,
      subjectEndByte: subject.endByte,
    };
  });
  return { findings };
};

const subjectsOverlap = (finding, diagnostic) =>
  finding.subjectStartByte < diagnostic.subjectEndByte &&
  diagnostic.subjectStartByte < finding.subjectEndByte;

const failures = [];
const results = [];

for (const [index, testCase] of ledger.cases.entries()) {
  const label = `${testCase.rule} [${index}]`;
  if (testCase.dialect !== "v1" && testCase.dialect !== "v2") {
    failures.push(`${label}: dialect must be exactly "v1" or "v2"`);
    continue;
  }
  if (testCase.dialect === "v2" && testCase.rule.startsWith("v1/")) {
    failures.push(`${label}: a v2 case cannot name a v1/ catalog rule`);
    continue;
  }
  if (testCase.sourceExtension !== undefined && !["ts", "tsx"].includes(testCase.sourceExtension)) {
    failures.push(`${label}: sourceExtension must be exactly "ts" or "tsx"`);
    continue;
  }
  if (testCase.compilerOptions !== undefined) {
    if (
      !testCase.compilerOptions ||
      Array.isArray(testCase.compilerOptions) ||
      typeof testCase.compilerOptions !== "object"
    ) {
      failures.push(`${label}: compilerOptions must be an object`);
      continue;
    }
    const unsupported = Object.keys(testCase.compilerOptions).filter(
      (name) => !CASE_COMPILER_OPTIONS.has(name),
    );
    if (unsupported.length) {
      failures.push(`${label}: unsupported compilerOptions ${unsupported.join(", ")}`);
      continue;
    }
    if (
      testCase.compilerOptions.verbatimModuleSyntax !== undefined &&
      typeof testCase.compilerOptions.verbatimModuleSyntax !== "boolean"
    ) {
      failures.push(`${label}: compilerOptions.verbatimModuleSyntax must be boolean`);
      continue;
    }
  }
  for (const field of ["presets", "enableRules"]) {
    if (
      testCase[field] !== undefined &&
      (!Array.isArray(testCase[field]) || testCase[field].some((value) => typeof value !== "string"))
    ) {
      failures.push(`${label}: ${field} must be an array of strings`);
      continue;
    }
  }
  const expectedRule = canonicalRule(testCase);
  if (!EXPECTATIONS.has(testCase.expect)) {
    failures.push(`${label}: unknown expectation ${JSON.stringify(testCase.expect)}`);
    continue;
  }
  if (!testCase.why || testCase.why.length < 20) {
    failures.push(`${label}: every case needs a written 'why'`);
    continue;
  }
  const result = runOracle(
    testCase.dialect,
    [{ name: sourceName(testCase, index), code: testCase.code }],
    testCase.compilerOptions,
  );
  const perPass = passes(result).map(([name, diagnostics]) => [name, errorsOnly(diagnostics)]);
  const seen = [...new Set(perPass.flatMap(([, diagnostics]) => diagnostics.map((d) => d.code)))].sort(
    (a, b) => a - b,
  );
  // The second half of the case: what the *checker* says about the same bytes.
  // Without it a `silent` case proves only that TypeScript is quiet, which an
  // over-narrowed rule that reports nothing at all satisfies just as well.
  const checkerPasses = [
    ["strict", runChecker(testCase, index, true)],
    ["loose", runChecker(testCase, index, false)],
  ];
  const targetByPass = checkerPasses.map(([name, observed]) => [
    name,
    observed.findings.filter((finding) => finding.rule === expectedRule),
  ]);
  const observedRules = [
    ...new Set(checkerPasses.flatMap(([, observed]) => observed.findings.map((finding) => finding.rule))),
  ].sort();
  results.push({
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
  });

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
    continue;
  }

  // The remaining expectations all require a diagnostic, and require it to be
  // one the ledger already accounted for. A *new* code means the case changed
  // meaning, which has to be re-explained rather than absorbed.
  const allow = new Set(testCase.allow ?? []);
  if (!allow.size) {
    failures.push(`${label}: ${testCase.expect} requires 'allow' listing the diagnostic codes`);
    continue;
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
    continue;
  }
  const unexpected = seen.filter((code) => !allow.has(code));
  if (unexpected.length) {
    failures.push(
      `${label}: unexpected ${unexpected.map((c) => `TS${c}`).join("/")} (declared: ` +
        `${[...allow].map((c) => `TS${c}`).join("/")}); re-explain the case`,
    );
  }
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

const catalogRules = [
  ...JSON.parse(readFileSync(join(ROOT, "packages/cli/lib/rules-solid-v1.json"), "utf8")).rules,
  ...JSON.parse(readFileSync(join(ROOT, "packages/cli/lib/rules-solid-v2.json"), "utf8")).rules,
].map((rule) => rule.name);

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
      ` (TypeScript and the checker); ${keystones} rule(s) carry a silent-tsc/reporting-rule keystone`,
  );
}

process.exit(failures.length ? 1 : 0);
