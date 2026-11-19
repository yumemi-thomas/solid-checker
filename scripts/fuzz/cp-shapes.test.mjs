import { describe, expect, it } from "vitest";
import { SPEC } from "./misuse.mjs";
import { generateShape, recallShapes } from "./matrix.mjs";
import { CP_CODES, CP_CONTEXT_NAMES } from "./cp-shapes.mjs";

// ADR 1123, the review's F2 and F3: a flushing child of a provider-like element whose value a tracked effect reads.
const CODE = "FLUSH_IN_TRACKED_QUEUE_CALLBACK";
const textOf = (context) => generateShape({ seed: 1 }, { code: CODE, context }, recallShapes(CODE, context)[0], 0).built.files["App.tsx"];

describe("the cp contexts", () => {
  it("are three misuse recall contexts of the flush code, one base shape each", () => {
    expect(CP_CODES).toEqual([CODE]);
    expect(CP_CONTEXT_NAMES).toEqual(["cp-stored", "cp-pattern", "cp-let"]);
    for (const context of CP_CONTEXT_NAMES) {
      expect(SPEC[CODE].recallContexts).toContain(context);
      expect(recallShapes(CODE, context).map((shape) => `${shape.binding}/${shape.layer}/${shape.source ?? ""}`)).toEqual(["direct/none/"]);
    }
  });
  it("write the site in a module-level child of the element, read in a tracked effect", () => {
    expect(textOf("cp-stored")).toMatch(/const pe = <CpC1 value=\{1\}><CpK1 a=\{1\} \/><\/CpC1>; createTrackedEffect\(/);
    expect(textOf("cp-pattern")).toMatch(/const \{ defaultValue: CpC1 = CpR1 \} = createContext\(CpR1\)/);
    expect(textOf("cp-let")).toMatch(/let CpC1 = createContext\(1\);\nCpC1 = CpR1 as unknown as typeof CpC1/);
    for (const context of CP_CONTEXT_NAMES) expect(textOf(context)).toMatch(/function CpK1\(_props: \{ a\?: unknown \}\) \{\n\s*flush\(\);/);
  });
});
