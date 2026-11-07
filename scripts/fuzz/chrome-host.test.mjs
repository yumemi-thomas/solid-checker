import { describe, expect, it } from "vitest";
import { mapPositions } from "./chrome-host.mjs";
import { INSTRUMENT_PRELUDE, cleanPositions } from "./hits.mjs";

/** A server whose one module's source map is the identity: what is left is the twin-to-clean step. */
const server = { moduleGraph: { getModuleByUrl: async () => ({ transformResult: { map: { mappings: "x" } } }) } };
const trace = { TraceMap: class {}, originalPositionFor: (_, { line, column }) => ({ line, column }) };

describe("mapPositions on an instrumented twin (ADR 0815)", () => {
  const run = `function f() {\n  __h(2); useIt();\n}\n${INSTRUMENT_PRELUDE}`;
  const text = "function f() {\n  useIt();\n}\n";

  it("gives positions on the clean text, as the checker's findings are", async () => {
    const record = { diagnostics: [{ code: "X", positions: [{ line: 2, column: 11 }] }], errors: [{ positions: [{ line: 2, column: 11 }] }] };
    const mapped = await mapPositions(server, trace, "a", record, run, cleanPositions(run, text));
    expect(mapped.diagnostics[0].positions).toEqual([{ file: "App.tsx", line: 2, column: 3 }]);
    expect(mapped.errors[0].positions).toEqual([{ file: "App.tsx", line: 2, column: 3 }]);
  });

  it("drops a frame in the counter's prelude: the clean text has no such line", async () => {
    const record = { diagnostics: [{ code: "X", positions: [{ line: 7, column: 3 }, { line: 2, column: 11 }] }] };
    const mapped = await mapPositions(server, trace, "a", record, run, cleanPositions(run, text));
    expect(mapped.diagnostics[0].positions).toEqual([{ file: "App.tsx", line: 2, column: 3 }]);
  });

  it("leaves a served clean text's positions as they are", async () => {
    const record = { diagnostics: [{ code: "X", positions: [{ line: 2, column: 3 }] }] };
    const mapped = await mapPositions(server, trace, "a", record, text);
    expect(mapped.diagnostics[0].positions).toEqual([{ file: "App.tsx", line: 2, column: 3 }]);
  });
});
