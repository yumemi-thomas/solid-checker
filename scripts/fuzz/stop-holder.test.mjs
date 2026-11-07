import { describe, expect, it } from "vitest";
import { STOP_VARIANTS, holderFree, stopPlan, stopProblems } from "./stop-holder.mjs";
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
    const plan = stopPlan(rows, (row) => ({ text: texts[row.id], others: 0 }));
    expect(plan.unmatched).toEqual(["b"]);
    expect(stopProblems(plan, new Map([["a", [{ kind: "uncertifiable" }]], ["c", []], ["d", null]]))).toEqual([
      "b: the holder-free transform matched nothing", "d: the checker gave no result", "c: certified with the holder removed, so its U was the holder's",
    ]);
  });
  it("counts a program with other files as unchecked, and never as passing", () => {
    const plan = stopPlan([{ id: "a", variant: "proto-literal" }], () => ({ text: "x", others: 1 }));
    expect(plan).toEqual({ checked: [], unmatched: [], unchecked: ["a"] });
    expect(stopProblems(plan, new Map())).toEqual([]);
  });
  it("leaves a stop row with no holder alone", () => {
    expect(stopPlan([{ id: "a", variant: null }, { id: "b", variant: "other" }], () => { throw new Error("read"); })).toEqual({ checked: [], unmatched: [], unchecked: [] });
  });
});
