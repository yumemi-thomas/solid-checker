// How an eslint-plugin-solid test case becomes a file the checker can lint.
//
// Shared by `scripts/parity.mjs`, which runs the checker over the corpus, and
// `scripts/parity-tsc-ownership.mjs`, which compiles the *same bytes* against
// the real published Solid typings. The two must agree on what a case is or the
// ownership answer describes different source than the parity answer.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
export const CORPUS = join(ROOT, "fixtures/upstream-parity");

export const corpusCases = () =>
  JSON.parse(readFileSync(join(CORPUS, "upstream-cases.json"), "utf8"));

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

export const withSolidImports = (code) => {
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

// The exact bytes a case is linted as, plus the two harness affixes that
// surround the case's own text. Keeping the affixes lets the fix-output
// comparison undo the harness precisely — no guessing at which leading lines
// were the harness's and which the case's.
export const materialize = (rule, testCase) => {
  // `jsx-no-undef` is about names that are not defined, so supplying the
  // import it tests for would delete the case.
  const code = rule === "jsx-no-undef" ? testCase.code : withSolidImports(testCase.code);
  // A file with no import or export is a *script* to TypeScript, so its
  // top-level names join a global scope shared with every other case —
  // one case's `Component` would define another's undefined reference.
  // Upstream lints each case as its own file; `export {}` restores that.
  const scoped = /^\s*(?:import|export)\b/m.test(code) ? code : `${code}\nexport {};`;
  return {
    source: `${scoped}\n`,
    // `withSolidImports` only ever prepends whole lines, so what precedes
    // the case's own text is exactly the imports it supplied.
    prefix: code.slice(0, code.length - testCase.code.length),
    suffix: `${scoped.slice(code.length)}\n`,
  };
};

/** The `<rule>__<valid|invalid>__NN` id parity keys every ledger entry by. */
export const caseId = (rule, kind, index) =>
  `${rule}__${kind}__${String(index).padStart(2, "0")}`;
