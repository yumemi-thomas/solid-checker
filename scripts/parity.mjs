// Runs every eslint-plugin-solid test case against the checker's v1 rules.
//
// The corpus is `fixtures/upstream-parity/upstream-cases.json` — 465 cases
// extracted from upstream's own suites (see the README there). Each case is
// materialised as its own file in one synthetic project, the checker runs
// once, and a case passes when the rule under test fired for an `invalid`
// case and stayed silent for a `valid` one.
//
// Deviations are declared in `deviations.json`, one entry per case with a
// reason. The comparison is exact in both directions: a case that starts
// deviating fails, and so does one that stops, so an inherited false positive
// cannot arrive quietly and a fix cannot silently rot.
//
//   node scripts/parity.mjs             compare against the declared deviations
//   node scripts/parity.mjs --update    rewrite deviations.json from this run
//
// `--update` keeps existing reasons and marks anything new `TRIAGE:`, which
// the comparison rejects — a new deviation has to be explained, not recorded.
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const CORPUS = join(ROOT, "fixtures/upstream-parity");
const update = process.argv.includes("--update");
const corpus = JSON.parse(readFileSync(join(CORPUS, "upstream-cases.json"), "utf8"));
const declared = existsSync(join(CORPUS, "deviations.json"))
  ? JSON.parse(readFileSync(join(CORPUS, "deviations.json"), "utf8"))
  : {};

// Upstream's `reactivity` is one rule behind eight message ids; the checker
// splits it. Mirrors the table in docs/rules/README.md.
const REACTIVITY = {
  untrackedReactive: ["v1/strict-read-untracked", "v1/reactive-read-after-await", "v1/no-destructure", "v1/untracked-derived-function"],
  badSignal: ["v1/uncalled-accessor"],
  badUnnamedDerivedSignal: ["v1/untracked-derived-function"],
  expectedFunctionGotExpression: ["v1/expected-function-got-expression"],
  noWrite: ["v1/no-direct-mutation"],
  noAsyncTrackedScope: ["v1/no-async-tracked-scope"],
  shouldDestructure: ["v1/reactive-source-uncaptured"],
  shouldAssign: ["v1/reactive-source-uncaptured"],
};
const ALL_REACTIVITY = [...new Set(Object.values(REACTIVITY).flat())];

// Upstream's cases call Solid's primitives without importing them: its rules
// match the name, so the import was never needed. The checker resolves a
// primitive through its import instead — stricter, and the reason a local
// function named `createEffect` is not mistaken for Solid's — so a case that
// never imports would exercise resolution rather than the rule. Supplying the
// import upstream assumes puts each case back on the rule it was written for.
const SOLID = {
  "solid-js": "createSignal createEffect createMemo createComputed createRenderEffect createDeferred createReaction createResource createRoot createContext useContext untrack batch on onMount onCleanup onError runWithOwner getOwner mergeProps splitProps children startTransition useTransition lazy mapArray indexArray For Show Index Switch Match Suspense SuspenseList ErrorBoundary".split(" "),
  "solid-js/store": "createStore createMutable produce reconcile unwrap modifyMutable".split(" "),
  "solid-js/web": "render hydrate isServer Portal Dynamic".split(" "),
};

const withSolidImports = (code) => {
  const lines = [];
  for (const [module, names] of Object.entries(SOLID)) {
    const needed = names.filter(
      (name) =>
        new RegExp(`\\b${name}\\b`).test(code) &&
        !new RegExp(`import[^;]*\\b${name}\\b[^;]*from`, "s").test(code) &&
        !new RegExp(`(?:const|let|var|function|class)\\s+${name}\\b`).test(code),
    );
    if (needed.length) lines.push(`import { ${needed.join(", ")} } from "${module}";`);
  }
  return lines.length ? `${lines.join("\n")}\n${code}` : code;
};

const project = join(ROOT, "rust/target/upstream-parity");
rmSync(project, { recursive: true, force: true });
mkdirSync(project, { recursive: true });
for (const [path, body] of Object.entries(JSON.parse(readFileSync(join(CORPUS, "harness.json"), "utf8")))) {
  mkdirSync(join(project, dirname(path)), { recursive: true });
  writeFileSync(join(project, path), body);
}

const cases = [];
for (const rule of corpus) {
  for (const kind of ["valid", "invalid"]) {
    rule[kind].forEach((testCase, index) => {
      const id = `${rule.rule}__${kind}__${String(index).padStart(2, "0")}`;
      // `jsx-no-undef` is about names that are not defined, so supplying the
      // import it tests for would delete the case.
      const code =
        rule.rule === "jsx-no-undef" ? testCase.code : withSolidImports(testCase.code);
      // A file with no import or export is a *script* to TypeScript, so its
      // top-level names join a global scope shared with every other case —
      // one case's `Component` would define another's undefined reference.
      // Upstream lints each case as its own file; `export {}` restores that.
      const scoped = /^\s*(?:import|export)\b/m.test(code) ? code : `${code}\nexport {};`;
      writeFileSync(join(project, `${id}.tsx`), `${scoped}\n`);
      cases.push({ id, rule: rule.rule, kind, ...testCase });
    });
  }
}

const output = execFileSync(join(ROOT, "bin/solid-checker-rust"), [
  "--format", "json", "--project", join(project, "tsconfig.json"),
], {
  env: { ...process.env, SOLID_TYPEFACTS_BIN: process.env.SOLID_TYPEFACTS_BIN ?? join(ROOT, "bin/solid-typefacts") },
  encoding: "utf8",
  maxBuffer: 1 << 28,
});

const fired = new Map();
for (const finding of JSON.parse(output).findings ?? []) {
  const id = finding.primaryLocation.path.split("/").pop().replace(/\.tsx$/, "");
  if (!fired.has(id)) fired.set(id, new Set());
  fired.get(id).add(finding.rule);
}

const observed = {};
let matched = 0;
for (const testCase of cases) {
  const rules = fired.get(testCase.id) ?? new Set();
  const relevant =
    testCase.rule === "reactivity" ? ALL_REACTIVITY : [`v1/${testCase.rule}`];
  const expected =
    testCase.rule === "reactivity" && testCase.messageIds.length
      ? [...new Set(testCase.messageIds.flatMap((id) => REACTIVITY[id] ?? []))]
      : relevant;
  const wanted = testCase.kind === "invalid";
  const got = [...rules].some((rule) => (wanted ? expected : relevant).includes(rule));
  if (got === wanted) {
    matched++;
    continue;
  }
  observed[testCase.id] = declared[testCase.id] ?? { status: "triage", reason: "" };
}

const total = cases.length;
console.log(`${matched}/${total} upstream cases match; ${Object.keys(observed).length} deviate`);

if (update) {
  const sorted = Object.fromEntries(Object.entries(observed).sort(([a], [b]) => a.localeCompare(b)));
  writeFileSync(join(CORPUS, "deviations.json"), `${JSON.stringify(sorted, null, 2)}\n`);
  const triage = Object.values(sorted).filter((entry) => entry.status === "triage").length;
  console.log(`recorded ${Object.keys(sorted).length} deviations${triage ? `, ${triage} still to triage` : ""}`);
  process.exit(0);
}

const appeared = Object.keys(observed).filter((id) => !(id in declared));
const resolved = Object.keys(declared).filter((id) => !(id in observed));
const untriaged = Object.entries(observed).filter(([, entry]) => entry.status === "triage" || !entry.reason);
for (const id of appeared) console.error(`new deviation, not declared: ${id}`);
for (const id of resolved) console.error(`declared deviation no longer deviates, remove it: ${id}`);
for (const [id] of untriaged) console.error(`deviation has no status or reason: ${id}`);
if (appeared.length || resolved.length || untriaged.length) {
  console.error("\nre-run with --update once every deviation above is triaged");
  process.exit(1);
}
const byStatus = {};
for (const entry of Object.values(declared)) byStatus[entry.status] = (byStatus[entry.status] ?? 0) + 1;
const summary = Object.entries(byStatus).sort().map(([k, v]) => `${v} ${k}`).join(", ");
console.log(`all ${Object.keys(declared).length} deviations declared: ${summary}`);
