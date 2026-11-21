import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { CHECKBOX_RETYPES, SPEC } from "./misuse.mjs";
import { recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";

// ADR 1148 (round 2, N1): the cells of a literal checkbox whose `type` the program rewrites before the user's step, one per spelling of the write, the escaped
// ones being the re-check's finding (a name the engine decodes and no source text spells).

const CODES = ["REACTIVE_WRITE_IN_OWNED_SCOPE", "STRICT_READ_UNTRACKED"];
const textOf = (code, context) => {
  const shape = recallShapes(code, context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code, context, variant: shape.variant, binding: "direct", layer: "none", legal: false } });
  expect(isValid(program), context).toBe(true);
  const built = buildCase(program, { id: `cb-${context}` });
  expect(built.sites, context).toHaveLength(1);
  return built.text.split("\n").map((line) => line.trim());
};

describe("the checkbox retype contexts", () => {
  it("are eight spellings of the write of `type`, one plain and seven that write a name with an escape or a method", () => {
    expect(Object.keys(CHECKBOX_RETYPES)).toEqual([
      "dny-change-checkbox-retyped",
      "dny-change-checkbox-esc-member",
      "dny-change-checkbox-esc-string",
      "dny-change-checkbox-esc-method",
      "dny-change-checkbox-esc-collection",
      "dny-change-checkbox-esc-collection-ident",
      "dny-change-checkbox-esc-collection-template",
      "dny-change-checkbox-esc-xpath",
    ]);
    // Every escaped spelling writes a backslash the source text holds and the decoded name no text holds: the shape of the finding.
    for (const [context, retype] of Object.entries(CHECKBOX_RETYPES)) expect(retype.includes("\\u"), context).toBe(context.includes("-esc-"));
  });

  it("are recall contexts of the two codes the retyped control's owned `change` delivery reports", () => {
    for (const code of CODES) {
      for (const context of Object.keys(CHECKBOX_RETYPES)) {
        expect(SPEC[code].recallContexts, `${code} ${context}`).toContain(context);
        expect(recallShapes(code, context).length, `${code} ${context}`).toBeGreaterThan(0);
      }
    }
  });

  it("print a literal checkbox with a `change` handler, the write of its `type`, then a focus, typed text and the later root that disables it", () => {
    for (const code of CODES) {
      for (const [context, retype] of Object.entries(CHECKBOX_RETYPES)) {
        const text = textOf(code, context);
        const line = (needle) => text.findIndex((one) => one.includes(needle));
        // The control: an intrinsic input with the one literal type and a change handler (the element ADR 1148 permits).
        expect(text.some((one) => /^<input data-act="[^"]+" type="checkbox" onChange=\{/.test(one)), `${code} ${context}`).toBe(true);
        // The write, written as the context says, runs before the focus and the typed text, in the timer that precedes the later root.
        const written = line(retype.replace(/^EL/, ""));
        expect(written, `${code} ${context}: the write is printed`).toBeGreaterThan(-1);
        expect(written, `${code} ${context}: before the focus`).toBeLessThan(line(".focus()"));
        expect(line(".focus()"), `${code} ${context}: before the typed text`).toBeLessThan(line('execCommand("insertText"'));
        expect(line("disabled = true"), `${code} ${context}: the later root disables it`).toBeGreaterThan(-1);
      }
    }
  });
});
