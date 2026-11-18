import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { censusCells, recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { PU_CODES, PU_CONTEXTS, PU_CONTEXT_NAMES, puShape } from "./pu-shapes.mjs";
import { plans } from "./api-fire-plan.mjs";

// ADR 1090 G1 (the Fable review of be4990c64): a read after an await in a function whose promise looks unconsumed, which an async memo still awaits.

const textOf = (context) => {
  const code = "UNTRACKED_READ_AFTER_AWAIT";
  const shape = recallShapes(code, context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code, context, variant: shape.variant, binding: "direct", layer: "none", legal: false } });
  const built = buildCase(program, { id: `pu-${context}` });
  expect(isValid(program), context).toBe(true);
  expect(built.neutral, context).toBe(true);
  expect(built.sites, context).toHaveLength(1);
  return built.text.split("\n").map((line) => line.trim());
};

describe("the pu contexts (a flight the proof took for unconsumed)", () => {
  it("are six misuse recall contexts of UNTRACKED_READ_AFTER_AWAIT, base-only, out of the census and api-fire's plan", () => {
    expect(PU_CONTEXT_NAMES).toEqual(["pu-then-sub", "pu-loop", "pu-class", "pu-coerce-direct", "pu-coerce-wrapper", "pu-coerce-iife"]);
    expect(PU_CODES).toEqual(["UNTRACKED_READ_AFTER_AWAIT"]);
    for (const name of PU_CONTEXT_NAMES) {
      expect(PU_CONTEXTS[name]).toBe("misuse");
      expect(SPEC.UNTRACKED_READ_AFTER_AWAIT.recallContexts).toContain(name);
      expect(BASE_ONLY_CONTEXTS()).toContain(name);
    }
    expect(censusCells({ codes: ["UNTRACKED_READ_AFTER_AWAIT"] }).some((cell) => PU_CONTEXT_NAMES.includes(cell.context))).toBe(false);
    expect(plans().some((plan) => PU_CONTEXT_NAMES.includes(plan.context))).toBe(false);
    expect(puShape("pu-loop", { binding: "alias", layer: "none" })).toBe(false);
    expect(puShape("pu-loop", { binding: "direct", layer: "none" })).toBe(true);
    expect(puShape("memo-async", { binding: "alias", layer: "closure" })).toBe(true);
  });
  it("pu-then-sub: the handler of a discarded `.then` on a Promise subclass that keeps its instances, at module scope; the component's memo awaits the kept one", () => {
    const lines = textOf("pu-then-sub");
    const subclass = lines.findIndex((line) => /^class Kp\d+ extends Promise<void> \{ static last\?: Promise<unknown>;.*Kp\d+\.last = this; \} \};?$/.test(line));
    const then = lines.findIndex((line) => /^Kp\d+\.resolve\(\)\.then\(async \(\) => \{/.test(line));
    const component = lines.findIndex((line) => line.startsWith("export function C1"));
    expect(subclass).toBeGreaterThanOrEqual(0);
    expect(then).toBeGreaterThan(subclass);
    expect(component).toBeGreaterThan(then);
    expect(lines.findIndex((line) => /^await Kp\d+\.last;$/.test(line))).toBeGreaterThan(component);
  });
  it("pu-loop: the site in a loop that awaits after it (a read in the synchronous prefix), the function called only from a `toString` the memo's template runs", () => {
    const lines = textOf("pu-loop");
    const fn = lines.findIndex((line) => /^const pf\d+ = async \(\) => \{$/.test(line));
    expect(fn).toBeGreaterThanOrEqual(0);
    expect(lines[fn + 1]).toBe("for (let i = 0; i < 2; i += 1) {");
    expect(lines.slice(fn, fn + 6)).toContain("await Promise.resolve();");
    expect(lines.some((line) => /^const po\d+ = \{ toString\(\) \{ pf\d+\(\); return ""; \} \};?$/.test(line))).toBe(true);
    expect(lines.some((line) => /^void `\$\{po\d+\}`;$/.test(line))).toBe(true);
  });
  it("pu-class: a helper holding the site, called by a class's instance field initializer written after the function's await; a `toString` the memo runs constructs it", () => {
    const lines = textOf("pu-class");
    expect(lines.some((line) => /^const ph\d+ = \(\) => \{$/.test(line))).toBe(true);
    const fn = lines.findIndex((line) => /^const pf\d+ = async \(\) => \{$/.test(line));
    expect(lines[fn + 1]).toBe("await Promise.resolve();");
    expect(lines[fn + 2]).toMatch(/^pb\d+\.K = class \{ x = ph\d+\(\); \};$/);
    expect(lines.some((line) => /^const po\d+ = \{ toString\(\) \{ new pb\d+\.K!\(\); return ""; \} \};?$/.test(line))).toBe(true);
  });
  it("pu-coerce-*: a setter call after the memo's await whose argument coerces an object whose own toString holds the site", () => {
    for (const [context, call] of [["pu-coerce-direct", /^pw\d+\(`\$\{po\d+\}`\);$/], ["pu-coerce-wrapper", /^pv\d+\(\);$/], ["pu-coerce-iife", /^\(\(\) => \{ pw\d+\(`\$\{po\d+\}`\); \}\)\(\);$/]]) {
      const lines = textOf(context);
      expect(lines.some((line) => /^const po\d+ = \{ toString: \(\) => \{$/.test(line)), context).toBe(true);
      expect(lines.some((line) => call.test(line)), context).toBe(true);
    }
  });
});
