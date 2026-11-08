import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { describe, expect, it } from "vitest";
import { readStopSource, STOP_VARIANTS, holderFree, stopPlan, stopProblems } from "./stop-holder.mjs";
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
  it("fails a transform that matches nothing, a run with no result, and a certified program", () => {
    const rows = [{ id: "a", variant: "proto-literal" }, { id: "b", variant: "proto-literal" }, { id: "c", variant: "proto-literal" }, { id: "d", variant: "proto-literal" }];
    const texts = { a: "(({ __proto__: s1 }) as any).x = 7", b: "s1.x = 7;", c: "(({ __proto__: s1 }) as any).x = 7", d: "(({ __proto__: s1 }) as any).x = 7" };
    const plan = stopPlan(rows, (row) => ({ files: { "App.tsx": texts[row.id] } }));
    expect(plan.unmatched).toEqual(["b"]);
    expect(stopProblems(plan, new Map([["a", [{ kind: "uncertifiable" }]], ["c", []], ["d", null]]))).toEqual([
      "b: the holder-free transform matched nothing", "d: the checker gave no result", "c: certified with the holder removed, so its U was the holder's",
    ]);
  });
  it("counts a program with no App.tsx as unchecked, and never as passing", () => {
    const plan = stopPlan([{ id: "a", variant: "proto-literal" }], () => ({ files: { "helpers.ts": "(({ __proto__: s1 }) as any).x = 7" } }));
    expect(plan).toEqual({ checked: [], unmatched: [], unchecked: ["a"] });
    expect(stopProblems(plan, new Map())).toEqual([]);
  });
  it("transforms every file of a multi-file program: the holder wherever it is, the other files unchanged; no file matched fails", () => {
    const helper = "export function h1(cb: () => unknown) {\n  cb();\n  return 1;\n}\n";
    const inHelper = stopPlan([{ id: "a", variant: "proto-literal" }], () => ({ files: { "App.tsx": "h1(() => 0);", "helpers.ts": "export const f = () => (({ __proto__: s1 }) as any).x = 7;" } }));
    expect(inHelper.checked).toEqual([{ id: "a", variant: "proto-literal", files: { "App.tsx": "h1(() => 0);", "helpers.ts": "export const f = () => s1.x = 7;" } }]);
    const none = stopPlan([{ id: "b", variant: "proto-literal" }], () => ({ files: { "App.tsx": "s1.x = 7;", "helpers.ts": helper } }));
    expect(none).toEqual({ checked: [], unmatched: ["b"], unchecked: [] });
    expect(stopProblems(none, new Map())).toEqual(["b: the holder-free transform matched nothing"]);
  });
  it("leaves a stop row with no holder alone", () => {
    expect(stopPlan([{ id: "a", variant: null }, { id: "b", variant: "other" }], () => { throw new Error("read"); })).toEqual({ checked: [], unmatched: [], unchecked: [] });
  });
});


// A real multi-file stop row (pinned smoke at 747594d6a, mx-recall): its holder in App.tsx, its helper in helpers.ts.
const REAL_ID = "mx-reactive-write-in-owned-scope-flow-match-installed-set-alias-direct-module-helper";
const REAL_APP = "import { Match, Switch, createSignal } from \"solid-js\";\nimport { h1 } from \"./helpers\";\n\nexport function C1() {\n  const [w1, setW1] = createSignal(0);\n  return (\n    <div>\n      <Switch>\n        <Match when={1}>{(_u) => {\n          h1(() => {\n            (() => { const t: any = {}; const dp = Object.defineProperty; dp(t, \"a\", { set: setW1 }); t.a = 7; })();\n          });\n          return <p data-view=\"v1\">x</p>;\n        }}</Match>\n      </Switch>\n    </div>\n  );\n}\n";
const REAL_HELPERS = "export function h1(cb: () => unknown) {\n  cb();\n  return 1;\n}\n";

describe("stop source discovery", () => {
  it.each(["helper.ts", "helper.tsx", "helper.mts", "helper.cts", "helper.js", "helper.jsx", "helper.mjs", "helper.cjs"])("reads %s from real files beside App.tsx, and nothing else", (sibling) => {
    const folder = mkdtempSync(join(tmpdir(), "stop-siblings-"));
    try {
      writeFileSync(join(folder, "App.tsx"), "(({ __proto__: s1 }) as any).x = 7");
      writeFileSync(join(folder, sibling), "export const helper = 1;");
      writeFileSync(join(folder, "notes.txt"), "ignored");
      writeFileSync(join(folder, "App.run.txt"), "ignored");
      writeFileSync(join(folder, "oracle.json"), "{}");
      expect(Object.keys(readStopSource(folder).files).sort()).toEqual(["App.tsx", sibling].sort());
      const plan = stopPlan([{ id: "a", variant: "proto-literal" }], () => readStopSource(folder));
      expect(plan.checked).toEqual([{ id: "a", variant: "proto-literal", files: { "App.tsx": "s1.x = 7", [sibling]: "export const helper = 1;" } }]);
    } finally { rmSync(folder, { recursive: true, force: true }); }
  });
  it("checks the real multi-file row with both files: the holder removed in App.tsx, helpers.ts as it was", () => {
    const folder = mkdtempSync(join(tmpdir(), "stop-real-"));
    try {
      writeFileSync(join(folder, "App.tsx"), REAL_APP);
      writeFileSync(join(folder, "helpers.ts"), REAL_HELPERS);
      const plan = stopPlan([{ id: REAL_ID, variant: "installed-set-alias" }], () => readStopSource(folder));
      expect(plan.unchecked).toEqual([]);
      expect(plan.unmatched).toEqual([]);
      expect(Object.keys(plan.checked[0].files).sort()).toEqual(["App.tsx", "helpers.ts"]);
      expect(plan.checked[0].files["helpers.ts"]).toBe(REAL_HELPERS);
      expect(plan.checked[0].files["App.tsx"]).toContain("            setW1(7);\n");
      expect(plan.checked[0].files["App.tsx"]).not.toContain("defineProperty");
      expect(stopProblems(plan, new Map([[REAL_ID, []]]))).toEqual([`${REAL_ID}: certified with the holder removed, so its U was the holder's`]);
      expect(stopProblems(plan, new Map([[REAL_ID, [{ kind: "uncertifiable" }]]]))).toEqual([]);
    } finally { rmSync(folder, { recursive: true, force: true }); }
  });
});

it("keeps every unchecked stop id blocking the final gate", () => {
  expect(finalBlockers({ rows: { grids: {}, stopUnchecked: ["a", "b"] } })).toEqual(["unchecked-stop/a", "unchecked-stop/b"]);
});
