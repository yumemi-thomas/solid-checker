// Who owns each upstream case: this checker, or TypeScript?
//
// `scripts/parity.mjs` records every divergence from eslint-plugin-solid in
// `deviations.json`, and 59 of them now carry `status: "policy"` with a prose
// reason of the form "TypeScript already reports this". Nothing checked that
// those reasons were true. A hand-written justification that no gate verifies is
// exactly the shape of the problem the absolute rule exists to fix — so this
// script compiles the *same bytes* parity lints, against the real published
// Solid 1.x typings, and holds the ledger to its own claims.
//
//   node scripts/parity-tsc-ownership.mjs           enforce the ledger's claims
//   node scripts/parity-tsc-ownership.mjs --report  print the partition
//
// One direction gates:
//
//   a `policy` deviation whose case TypeScript does *not* report
//       The reason is false. Either the rule should still fire there, or the
//       reason names the wrong mechanism. This caught two `class:mt-10` cases
//       declared covered by TS2322 when TypeScript in fact declines to check any
//       hyphenated JSX attribute name at all -- a hole in a narrowing that no
//       other gate could see.
//
// One direction reports:
//
//   an `invalid` case the checker still reports where TypeScript reports too
//       A *candidate* duplicate, printed under `--report` and deliberately not
//       a failure. This direction compares presence per file, and the upstream
//       corpus is untyped JavaScript: most cases carry incidental TS7006
//       implicit-any and TS2304 cannot-find-name diagnostics that have nothing
//       to do with the rule's span. Making it a gate needs the checker's finding
//       spans so the comparison can require overlap, which means parity emitting
//       a machine-readable run artifact. Until then it is a discovery list, and
//       calling it a gate would be the same mistake as trusting a prose reason.
//
// The partition it prints is a *lower bound on cleanliness*, not a claim about
// ownership, and the difference matters. It counts cases that do not type-check
// at all against the real typings -- which for an untyped upstream corpus is
// mostly incidental, not the rule's subject. Read it as "this many corpus cases
// are not valid TypeScript", which is worth knowing on its own, and read the
// candidate list below it as leads. Turning either into an ownership claim needs
// span overlap against the checker's own findings.
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { CORPUS, caseId, corpusCases, materialize } from "./lib/upstream-cases.mjs";
import { runOracle } from "./tsc-oracle.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const report = process.argv.includes("--report");

const deviations = JSON.parse(readFileSync(join(CORPUS, "deviations.json"), "utf8"));
const corpus = corpusCases();

// One program over the whole corpus. Per-case oracle runs would copy the
// audited install 465 times; the diagnostics are per file either way.
const inputs = [];
const meta = new Map();
for (const rule of corpus) {
  for (const kind of ["valid", "invalid"]) {
    rule[kind].forEach((testCase, index) => {
      const id = caseId(rule.rule, kind, index);
      const name = `${id}.tsx`;
      inputs.push({ name, code: materialize(rule.rule, testCase).source });
      meta.set(name, { id, rule: rule.rule, kind });
    });
  }
}

const result = runOracle("v1", inputs);
// Both passes: "only under `strict`" is not an exception the absolute rule
// recognises, so a diagnostic in either pass means TypeScript speaks.
const speaks = new Map();
for (const pass of ["strict", "loose"]) {
  for (const diagnostic of result.passes[pass]) {
    if (diagnostic.category !== "error") continue;
    if (!speaks.has(diagnostic.file)) speaks.set(diagnostic.file, []);
    speaks.get(diagnostic.file).push({ pass, ...diagnostic });
  }
}

const owned = (id) => speaks.get(`${id}.tsx`) ?? [];
const failures = [];

// Direction one: every `policy` reason must be true.
for (const [id, entry] of Object.entries(deviations.fired ?? {})) {
  if (entry.status !== "policy") continue;
  const diagnostics = owned(id);
  if (!diagnostics.length) {
    failures.push(
      `${id}: declared status "policy" — "${entry.reason.slice(0, 90)}…" — but TypeScript reports` +
        ` nothing on this case against solid-js@${result.versions["solid-js"]}.` +
        `\n    Either the rule should still fire here, or the reason names the wrong mechanism.`,
    );
  }
}

// Direction two: candidates only. An `invalid` case the checker still reports
// where TypeScript also reports *something in the same file*. Reported, never
// failed — see the header for why span overlap is required before this can gate.
// Codes that are pure artefacts of linting untyped upstream JavaScript are
// dropped, because they are never the rule's subject.
const INCIDENTAL = new Set([
  7006, // Parameter implicitly has an 'any' type
  7031, // Binding element implicitly has an 'any' type
  2304, // Cannot find name
  18004, // No value exists in scope for the shorthand property
  2554, // Expected N arguments
  2559, // Type has no properties in common
]);
const duplicates = [];
for (const { id, rule, kind } of meta.values()) {
  if (kind !== "invalid") continue;
  if (deviations.fired?.[id]) continue; // the checker is silent here
  const codes = [...new Set(owned(id).map((diagnostic) => diagnostic.code))]
    .filter((code) => !INCIDENTAL.has(code))
    .sort();
  if (codes.length) duplicates.push({ id, rule, codes });
}

if (report) {
  const perRule = new Map();
  for (const { id, rule, kind } of meta.values()) {
    if (!perRule.has(rule)) perRule.set(rule, { total: 0, typescript: 0, invalid: 0 });
    const row = perRule.get(rule);
    row.total += 1;
    if (kind === "invalid") row.invalid += 1;
    if (owned(id).length) row.typescript += 1;
  }
  console.log(`corpus against solid-js@${result.versions["solid-js"]}, TypeScript ${result.typescript}\n`);
  console.log("  cases  ts-owned  rule");
  for (const [rule, row] of [...perRule].sort((a, b) => b[1].typescript - a[1].typescript)) {
    console.log(
      `  ${String(row.total).padStart(5)}  ${String(row.typescript).padStart(8)}  ${rule}`,
    );
  }
  const total = meta.size;
  const tsOwned = [...meta.values()].filter(({ id }) => owned(id).length).length;
  console.log(
    `\n  ${tsOwned} of ${total} cases do not type-check against the real typings (${Math.round(
      (100 * tsOwned) / total,
    )}%) -- mostly incidentally, since the upstream corpus is untyped JavaScript.` +
      `\n  This is NOT "TypeScript owns the defect in these cases": that needs the` +
      ` diagnostic to overlap the finding's span, which this comparison does not check.`,
  );
  console.log(
    `\n  ${duplicates.length} candidate duplicate(s) -- the checker reports and so does TypeScript,` +
      ` incidental untyped-corpus codes excluded. Candidates, not findings: this` +
      ` comparison is per file, not per span.`,
  );
  for (const { id, rule, codes } of duplicates) {
    console.log(`    ${id} (${rule}) ${codes.map((code) => `TS${code}`).join("/")}`);
  }
}

if (failures.length) {
  console.error(`parity ownership: ${failures.length} problem(s)\n`);
  for (const failure of failures) console.error(`  - ${failure}\n`);
  process.exit(1);
}
console.log(
  `parity ownership: ${
    Object.values(deviations.fired ?? {}).filter((entry) => entry.status === "policy").length
  } "policy" reasons verified against solid-js@${result.versions["solid-js"]}` +
    ` (${duplicates.length} candidate duplicates; --report to list them)`,
);
if (!existsSync(join(ROOT, "fixtures/tsc-oracle/packages.json"))) process.exit(1);
