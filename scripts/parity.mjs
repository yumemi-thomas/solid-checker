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
// `--update` keeps existing reasons and marks anything new `triage`, which
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
  // A runWithOwner callback is a direct untracked read in the checker (the
  // runtime clears Listener); upstream calls the same defect an unnamed
  // derived signal. Either evidence-backed diagnosis satisfies that case.
  badUnnamedDerivedSignal: ["v1/untracked-derived-function", "v1/strict-read-untracked"],
  // Passing a reactive prop member where a callback is expected is also an
  // untracked read of that member. The checker may surface the more general
  // proof when the callback's callable type itself is unresolved.
  expectedFunctionGotExpression: ["v1/expected-function-got-expression", "v1/strict-read-untracked"],
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
const materializeCase = (directory, id, rule, testCase) => {
  // `jsx-no-undef` is about names that are not defined, so supplying the
  // import it tests for would delete the case.
  const code = rule === "jsx-no-undef" ? testCase.code : withSolidImports(testCase.code);
  // A file with no import or export is a *script* to TypeScript, so its
  // top-level names join a global scope shared with every other case —
  // one case's `Component` would define another's undefined reference.
  // Upstream lints each case as its own file; `export {}` restores that.
  const scoped = /^\s*(?:import|export)\b/m.test(code) ? code : `${code}\nexport {};`;
  writeFileSync(join(directory, `${id}.tsx`), `${scoped}\n`);
};
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
      const id = `${rule.rule}__${kind}__${String(index).padStart(2, "0")}`;
      const options = testCase.options?.[0];
      if (options && CONFIGURABLE.has(rule.rule)) {
        const key = `${rule.rule}::${JSON.stringify(options)}`;
        if (!configured.has(key)) configured.set(key, { rule: rule.rule, options, members: [] });
        configured.get(key).members.push({ id, testCase });
      } else {
        materializeCase(project, id, rule.rule, testCase);
      }
      cases.push({ id, rule: rule.rule, kind, ...testCase });
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
  for (const { id, testCase } of members) materializeCase(directory, id, rule, testCase);
  runs.push(runChecker(directory));
}

const fired = new Map();
for (const output of runs) {
  for (const finding of JSON.parse(output).findings ?? []) {
    // Separator-agnostic: the checker reports whichever separator the host
    // OS uses, and a Windows path never splits on "/".
    const id = finding.primaryLocation.path.split(/[\\/]/).pop().replace(/\.tsx$/, "");
    if (!fired.has(id)) fired.set(id, new Set());
    fired.get(id).add(finding.rule);
  }
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
// The full status vocabulary; the README's table documents each. A retired
// status (like `unsupported-option`, emptied when the option-bearing rules
// grew upstream's options surface) is removed here so it cannot quietly
// come back.
const allowedStatuses = new Set([
  "evidence-backed",
  "fact-unavailable",
  "policy",
]);
const untriaged = Object.entries(observed).filter(
  ([, entry]) => !allowedStatuses.has(entry.status) || !entry.reason,
);
for (const id of appeared) console.error(`new deviation, not declared: ${id}`);
for (const id of resolved) console.error(`declared deviation no longer deviates, remove it: ${id}`);
for (const [id] of untriaged) console.error(`deviation has an invalid status or no reason: ${id}`);
if (appeared.length || resolved.length || untriaged.length) {
  console.error("\nre-run with --update once every deviation above is triaged");
  process.exit(1);
}
const byStatus = {};
for (const entry of Object.values(declared)) byStatus[entry.status] = (byStatus[entry.status] ?? 0) + 1;
const summary = Object.entries(byStatus).sort().map(([k, v]) => `${v} ${k}`).join(", ");
console.log(`all ${Object.keys(declared).length} deviations declared: ${summary}`);
