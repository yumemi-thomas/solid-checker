import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { describe, expect, it } from "vitest";
import { readStopSource, STOP_VARIANTS, holderFree, mountAsApp, stopPlan, stopProblems, unresolvedImports } from "./stop-holder.mjs";
import { finalBlockers } from "./open-defects.mjs";
import { inStopGroup } from "./open-rows.mjs";

// The generator's own text of each stop variant (misuse.mjs), with `o` the site.
const GENERATED = {
  "installed-set-reflect": ['(() => { const t: any = {}; Reflect.defineProperty(t, "a", { set: setW1 }); t.a = 7; })()', "setW1(7)"],
  "installed-set-alias": ['(() => { const t: any = {}; const dp = Object.defineProperty; dp(t, "a", { set: setW1 }); t.a = 7; })()', "setW1(7)"],
  "store-proto-merge": ["merge(({ __proto__: s1 }) as any, { w: 2 }).v", "s1.v"],
  "store-proto-literal": ["(({ __proto__: s1 }) as any).v", "s1.v"],
  "store-proto-string": ['(({ "__proto__": s1 }) as any).v', "s1.v"],
  "store-proto-member": ["(() => { const t: any = { a: 1 }; t.__proto__ = s1; return t.v; })()", "s1.v"],
  "proto-key-handed": ['(() => { const t: any = { a: 1 }; const put = (k: string, v: unknown) => { t[k] = v; }; put("__proto__", s1); t.x = 7; })()', "s1.x = 7"],
  "proto-member": ["(() => { const t: any = { a: 1 }; t.__proto__ = s1; t.x = 7; })()", "s1.x = 7"],
  "proto-literal": ["(({ __proto__: s1 }) as any).x = 7", "s1.x = 7"],
  "proto-method-this": ["(() => { const t: any = { install() { Object.setPrototypeOf(this, s1); } }; t.install(); t.x = 7; })()", "s1.x = 7"],
};

describe("the stop rows' holder-free check", () => {
  it("has a transform for every stop variant, each taking the generator's text to the bare site", () => {
    expect(Object.keys(GENERATED).sort()).toEqual([...STOP_VARIANTS].sort());
    for (const [variant, [text, bare]] of Object.entries(GENERATED)) expect(holderFree(variant, `  ${text};\n`), variant).toBe(`  ${bare};\n`);
  });
  it("stops only on a variant it can transform", () => {
    expect(inStopGroup({ outcome: "false-clean", variant: "installed-set-alias" })).toBe(true);
    expect(inStopGroup({ outcome: "false-clean", variant: "no-transform-for-this" })).toBe(false);
  });
  // A program as the generator prints it: C1 in App.tsx, the holder on line 2. `at(line)` a finding the checker placed there.
  const program = (body) => `export function C1() {\n  ${body};\n  return null;\n}\n`;
  const at = (line, file = "App.tsx") => ({ kind: "uncertifiable", primaryLocation: { path: `/w/${file}`, line, column: 3 } });
  const site = { file: "App.tsx", line: 2 };
  it("fails a transform that matches nothing, a run with no result, a certified program, and a finding off the row's site", () => {
    const rows = ["a", "b", "c", "d", "e"].map((id) => ({ id, variant: "proto-literal", site }));
    const texts = { a: program("(({ __proto__: s1 }) as any).x = 7"), b: program("s1.x = 7"), c: program("(({ __proto__: s1 }) as any).x = 7"), d: program("(({ __proto__: s1 }) as any).x = 7"), e: program("(({ __proto__: s1 }) as any).x = 7") };
    const plan = stopPlan(rows, (row) => ({ files: { "App.tsx": texts[row.id] } }));
    expect(plan.unmatched).toEqual(["b"]);
    expect(plan.checked.find((row) => row.id === "a").files["App.tsx"]).toBe("export function App() {\n  s1.x = 7;\n  return null;\n}\n");
    expect(stopProblems(plan, new Map([["a", [at(2)]], ["c", []], ["d", null], ["e", [at(3), at(2, "helpers.ts")]]]))).toEqual([
      "b: the holder-free transform matched nothing in the site's file", "d: the checker gave no result", "c: certified with the holder removed, so its U was the holder's",
      "e: no finding at its site App.tsx:2 with the holder removed",
    ]);
  });
  it("counts a program with no App.tsx as unchecked, and never as passing", () => {
    const plan = stopPlan([{ id: "a", variant: "proto-literal", site }], () => ({ files: { "helpers.ts": "(({ __proto__: s1 }) as any).x = 7" } }));
    expect(plan).toEqual({ checked: [], unmatched: [], failed: [], unchecked: ["a"] });
    expect(stopProblems(plan, new Map())).toEqual([]);
  });
  it("needs the site's file among the transformed files: a holder only in a helper is unmatched", () => {
    const plan = stopPlan([{ id: "a", variant: "proto-literal", site }], () => ({ files: { "App.tsx": program("h1(() => 0)"), "helpers.ts": "export const f = () => (({ __proto__: s1 }) as any).x = 7;" } }));
    expect(plan.unmatched).toEqual(["a"]);
  });
  it("transforms every file: the holder wherever it is, the other files unchanged", () => {
    const plan = stopPlan([{ id: "a", variant: "proto-literal", site }], () => ({ files: { "App.tsx": program("(({ __proto__: s1 }) as any).x = 7"), "helpers.ts": "export const f = () => (({ __proto__: s2 }) as any).x = 7;" } }));
    expect(plan.checked[0].files).toEqual({ "App.tsx": "export function App() {\n  s1.x = 7;\n  return null;\n}\n", "helpers.ts": "export const f = () => s2.x = 7;" });
  });
  it("fails a program whose C1 the mount rename cannot find", () => {
    expect(mountAsApp("export const C1 = () => null;")).toBe(null);
    const plan = stopPlan([{ id: "a", variant: "proto-literal", site }], () => ({ files: { "App.tsx": "export const C1 = () => (({ __proto__: s1 }) as any).x = 7;" } }));
    expect(plan.failed).toEqual([{ id: "a", why: "App.tsx has no `export function C1()` to mount as App" }]);
    expect(stopProblems(plan, new Map())).toEqual(["a: App.tsx has no `export function C1()` to mount as App"]);
  });
  it("fails a program with a source file in a subdirectory, and one importing a local file it does not have", () => {
    const nested = stopPlan([{ id: "a", variant: "proto-literal", site }], () => ({ files: { "App.tsx": program("(({ __proto__: s1 }) as any).x = 7") }, nested: ["lib/h.ts"] }));
    expect(nested.failed).toEqual([{ id: "a", why: "source files in a subdirectory: lib/h.ts" }]);
    const missing = stopPlan([{ id: "b", variant: "proto-literal", site }], () => ({ files: { "App.tsx": `import { h1 } from "./helpers";\n${program("(({ __proto__: s1 }) as any).x = 7")}` } }));
    expect(missing.failed).toEqual([{ id: "b", why: "local imports naming no file of the program: App.tsx -> ./helpers" }]);
    expect(unresolvedImports({ "App.tsx": 'import { h1 } from "./helpers";\nexport * from "./more.js";\nconst m = import("./lazy");', "helpers.ts": "export const h1 = 1;", "more.ts": "", "lazy.tsx": "" })).toEqual(["App.tsx -> ./more.js"]);
    expect(unresolvedImports({ "App.tsx": 'import "./side";\nimport { x } from "solid-js";', "side.ts": "" })).toEqual([]);
  });
  it("leaves a stop row with no holder alone", () => {
    expect(stopPlan([{ id: "a", variant: null }, { id: "b", variant: "other" }], () => { throw new Error("read"); })).toEqual({ checked: [], unmatched: [], failed: [], unchecked: [] });
  });
});


const at = (line, file = "App.tsx") => ({ kind: "uncertifiable", primaryLocation: { path: `/w/${file}`, line, column: 3 } });
// A real multi-file stop row (pinned smoke at 747594d6a, mx-recall): its holder in App.tsx, its helper in helpers.ts.
const REAL_ID = "mx-reactive-write-in-owned-scope-flow-match-installed-set-alias-direct-module-helper";
const REAL_APP = "import { Match, Switch, createSignal } from \"solid-js\";\nimport { h1 } from \"./helpers\";\n\nexport function C1() {\n  const [w1, setW1] = createSignal(0);\n  return (\n    <div>\n      <Switch>\n        <Match when={1}>{(_u) => {\n          h1(() => {\n            (() => { const t: any = {}; const dp = Object.defineProperty; dp(t, \"a\", { set: setW1 }); t.a = 7; })();\n          });\n          return <p data-view=\"v1\">x</p>;\n        }}</Match>\n      </Switch>\n    </div>\n  );\n}\n";
const REAL_HELPERS = "export function h1(cb: () => unknown) {\n  cb();\n  return 1;\n}\n";

describe("stop source discovery", () => {
  it.each(["helper.ts", "helper.tsx", "helper.mts", "helper.cts", "helper.js", "helper.jsx", "helper.mjs", "helper.cjs"])("reads %s from real files beside App.tsx, and nothing else", (sibling) => {
    const folder = mkdtempSync(join(tmpdir(), "stop-siblings-"));
    try {
      writeFileSync(join(folder, "App.tsx"), "export function C1() {\n  (({ __proto__: s1 }) as any).x = 7;\n}\n");
      writeFileSync(join(folder, sibling), "export const helper = 1;");
      writeFileSync(join(folder, "notes.txt"), "ignored");
      writeFileSync(join(folder, "App.run.txt"), "ignored");
      writeFileSync(join(folder, "oracle.json"), "{}");
      expect(readStopSource(folder)).toEqual({ files: { "App.tsx": "export function C1() {\n  (({ __proto__: s1 }) as any).x = 7;\n}\n", [sibling]: "export const helper = 1;" }, nested: [] });
      const plan = stopPlan([{ id: "a", variant: "proto-literal", site: { file: "App.tsx", line: 2 } }], () => readStopSource(folder));
      expect(plan.checked[0].files).toEqual({ "App.tsx": "export function App() {\n  s1.x = 7;\n}\n", [sibling]: "export const helper = 1;" });
    } finally { rmSync(folder, { recursive: true, force: true }); }
  });
  it("finds a source file in a subdirectory on disk, and the plan refuses it", () => {
    const folder = mkdtempSync(join(tmpdir(), "stop-nested-"));
    try {
      writeFileSync(join(folder, "App.tsx"), "export function C1() {\n  (({ __proto__: s1 }) as any).x = 7;\n}\n");
      mkdirSync(join(folder, "lib", "deep"), { recursive: true });
      writeFileSync(join(folder, "lib", "deep", "h.ts"), "export const h = 1;");
      writeFileSync(join(folder, "lib", "notes.txt"), "ignored");
      expect(readStopSource(folder).nested).toEqual(["lib/deep/h.ts"]);
      expect(stopPlan([{ id: "a", variant: "proto-literal", site: { file: "App.tsx", line: 2 } }], () => readStopSource(folder)).failed).toEqual([{ id: "a", why: "source files in a subdirectory: lib/deep/h.ts" }]);
    } finally { rmSync(folder, { recursive: true, force: true }); }
  });
  it("checks the real multi-file row with both files: the holder removed in App.tsx, helpers.ts as it was, passing only on a finding at its site", () => {
    const folder = mkdtempSync(join(tmpdir(), "stop-real-"));
    try {
      writeFileSync(join(folder, "App.tsx"), REAL_APP);
      writeFileSync(join(folder, "helpers.ts"), REAL_HELPERS);
      // The ledger's blocker for this row: SC9012/installed-member@11:52 (the holder's line, the site's statement).
      const realSite = { file: "App.tsx", line: 11 };
      const plan = stopPlan([{ id: REAL_ID, variant: "installed-set-alias", site: realSite }], () => readStopSource(folder));
      expect(plan.unchecked).toEqual([]);
      expect(plan.unmatched).toEqual([]);
      expect(plan.failed).toEqual([]);
      expect(Object.keys(plan.checked[0].files).sort()).toEqual(["App.tsx", "helpers.ts"]);
      expect(plan.checked[0].files["helpers.ts"]).toBe(REAL_HELPERS);
      expect(plan.checked[0].files["App.tsx"].split("\n")[10]).toBe("            setW1(7);");
      expect(plan.checked[0].files["App.tsx"]).toContain("export function App()");
      expect(plan.checked[0].files["App.tsx"]).not.toContain("defineProperty");
      expect(stopProblems(plan, new Map([[REAL_ID, []]]))).toEqual([`${REAL_ID}: certified with the holder removed, so its U was the holder's`]);
      expect(stopProblems(plan, new Map([[REAL_ID, [at(11)]]]))).toEqual([]);
      expect(stopProblems(plan, new Map([[REAL_ID, [at(10), at(11, "helpers.ts")]]]))).toEqual([`${REAL_ID}: no finding at its site App.tsx:11 with the holder removed`]);
    } finally { rmSync(folder, { recursive: true, force: true }); }
  });
});

it("keeps every unchecked stop id blocking the final gate", () => {
  expect(finalBlockers({ rows: { grids: {}, stopUnchecked: ["a", "b"] } })).toEqual(["unchecked-stop/a", "unchecked-stop/b"]);
});
