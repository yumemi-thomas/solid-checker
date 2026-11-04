import { describe, expect, it } from "vitest";
import { stripInstrumentation, PRELUDE_MARK } from "./hits.mjs";
import { openWindows, silenceMeasured, windowProbeText } from "./window.mjs";

const twin = `import { flush } from "solid-js";\nfunction C1() { __h(1);\n  __h(2); read();\n}\n${PRELUDE_MARK}function __h(id: number): void {\n  count(id);\n}\n`;
const warn = (id) => ({ code: "STRICT_READ_UNTRACKED", message: `[STRICT_READ_UNTRACKED] Reactive value "__scWindow:${id}" read directly in a component body will not update.` });
const built = { sites: [{ id: "s1", line: 10, hit: 7 }], glue: [{ id: "g1", line: 12, hit: 8 }] };
const finding = { line: 10, endLine: 10 };

describe("the control run's text", () => {
  it("keeps every line number and reads one signal per counter, the program's own text untouched", () => {
    const probed = windowProbeText(twin);
    expect(probed.split("\n").length).toBe(twin.split("\n").length + 1);
    expect(probed.split("\n")[1]).toBe("function C1() { __h(1);");
    expect(probed).toContain("{ length: 3 }");
    expect(probed).toContain("__scWindow[id]();");
    expect(probed.split("\n").slice(1, 3)).toEqual(twin.split("\n").slice(1, 3));
    expect(stripInstrumentation(twin)).not.toContain("__scWindow");
  });
  it("refuses a text with no counters: a control that reads nothing would measure silence", () => {
    expect(() => windowProbeText("const a = 1;\n")).toThrow(/no counter prelude/);
  });
});

describe("what a control run measures", () => {
  it("reads the counters whose signal warned, and no other warning", () => {
    const record = { diagnostics: [warn(5), warn(9), { code: "STRICT_READ_UNTRACKED", message: `Reactive value "s1" read` }, { code: "STORE_WRITE_OUTSIDE_SETTER", message: `"__scWindow:3"` }] };
    expect([...openWindows(record)]).toEqual([5, 9]);
  });
  const run = (diagnostics, extra = {}) => ({ hits: { 7: 1, 8: 1 }, diagnostics, errors: [], ...extra });
  it("excuses a finding whose lines ran with no window open", () => {
    expect(silenceMeasured({ finding, built, record: run([]) })).toBe(true);
    // A window elsewhere says nothing about this line.
    expect(silenceMeasured({ finding, built, record: run([warn(8)]) })).toBe(true);
  });
  it("does not excuse one with a window open at its line: the claim is false", () => {
    expect(silenceMeasured({ finding, built, record: run([warn(7)]) })).toBe(false);
  });
  it("measures nothing, so excuses nothing, on no record, a failed or disturbed run, lines that did not run, or lines with no counter", () => {
    expect(silenceMeasured({ finding, built, record: undefined })).toBe(false);
    expect(silenceMeasured({ finding, built, record: run([], { failure: { phase: "x" } }) })).toBe(false);
    expect(silenceMeasured({ finding, built, record: run([], { errors: [{ kind: "page-error" }] }) })).toBe(false);
    expect(silenceMeasured({ finding, built, record: run([], { hits: { 7: 0, 8: 1 } }) })).toBe(false);
    expect(silenceMeasured({ finding, built, record: run([], { hits: null }) })).toBe(false);
    expect(silenceMeasured({ finding: { line: 99, endLine: 99 }, built, record: run([]) })).toBe(false);
  });
});
