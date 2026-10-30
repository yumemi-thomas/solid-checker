import { describe, expect, it } from "vitest";
import { DEFAULT_DIFF_AXES, parseAxes, parsePerStratum, runtimeDiff, sampleStrata, stratumOf, summarize } from "./runtime-diff.mjs";

const record = (overrides = {}) => ({
  failure: null,
  hits: { s1: 1, s2: 0 },
  diagnostics: [],
  errors: [],
  unattributed: [],
  witnesses: [{ siteId: "s1", witness: { selector: "#w1", text: "1" }, actual: "1" }],
  ...overrides,
});
const diagnostic = (code, line) => ({ code, positions: [{ file: "App.tsx", line }] });

describe("runtimeDiff", () => {
  it("finds nothing between equal records", () => {
    expect(runtimeDiff(record(), record())).toEqual([]);
  });

  it("names a diagnostic only Chrome reports, and one only Happy DOM reports", () => {
    const chromeOnly = runtimeDiff(record(), record({ diagnostics: [diagnostic("STRICT_READ_UNTRACKED", 7)] }));
    expect(chromeOnly).toEqual([{ kind: "chrome-only", detail: "STRICT_READ_UNTRACKED@7" }]);
    const happyOnly = runtimeDiff(record({ diagnostics: [diagnostic("NO_OWNER_EFFECT", 3)] }), record());
    expect(happyOnly).toEqual([{ kind: "happy-only", detail: "NO_OWNER_EFFECT@3" }]);
  });

  it("treats the same code on another line as two differences, and a thrown code like a diagnostic", () => {
    const moved = runtimeDiff(record({ diagnostics: [diagnostic("STRICT_READ_UNTRACKED", 7)] }), record({ diagnostics: [diagnostic("STRICT_READ_UNTRACKED", 8)] }));
    expect(moved).toEqual([{ kind: "moved", detail: "STRICT_READ_UNTRACKED happy@7 chrome@8" }]);
    const thrown = { message: "[FLUSH_IN_ACTION] Cannot call flush() inside an action", positions: [{ file: "App.tsx", line: 9 }] };
    const sameCode = runtimeDiff(record({ errors: [{ kind: "node-uncaught", ...thrown }] }), record({ diagnostics: [diagnostic("FLUSH_IN_ACTION", 9)] }));
    expect(sameCode.filter((d) => d.kind === "chrome-only" || d.kind === "happy-only")).toEqual([]);
  });

  it("counts an extra line of a code both sides report as behaviour: an event at a site one side never placed one at", () => {
    const one = record({ diagnostics: [diagnostic("CONTEXT_NOT_FOUND", 18)] });
    const two = record({ diagnostics: [diagnostic("CONTEXT_NOT_FOUND", 16), diagnostic("CONTEXT_NOT_FOUND", 18)] });
    expect(runtimeDiff(two, one)).toEqual([{ kind: "moved", detail: "CONTEXT_NOT_FOUND happy@16,18 chrome@18" }]);
    expect(summarize([{ diffs: runtimeDiff(two, one) }])).toMatchObject({ behaviour: 1, attributionOnly: 0 });
  });

  it("matches a positionless event to its code on the other side, and drops one the same side places", () => {
    const placed = record({ diagnostics: [diagnostic("FLUSH_IN_ACTION", 6)] });
    const twice = record({ diagnostics: [diagnostic("FLUSH_IN_ACTION", 6), { code: "FLUSH_IN_ACTION", positions: [] }] });
    expect(runtimeDiff(placed, twice)).toEqual([]);
    const unplaced = record({ diagnostics: [{ code: "FLUSH_IN_ACTION", positions: [] }] });
    expect(runtimeDiff(placed, unplaced)).toEqual([]);
    expect(runtimeDiff(record(), unplaced)).toEqual([{ kind: "chrome-only", detail: "FLUSH_IN_ACTION@-" }]);
  });

  it("ignores a consequence code's line, and a throw kind or message that is the host's", () => {
    const halted = (line) => record({ diagnostics: [diagnostic("REACTIVITY_HALTED", line)] });
    expect(runtimeDiff(halted(40), halted(900))).toEqual([]);
    const thrown = (kind, message) => record({ errors: [{ kind, message, positions: [] }] });
    expect(runtimeDiff(thrown("node-uncaught", "boom"), thrown("page-error", "Uncaught Error: boom"))).toEqual([]);
  });

  it("counts an unattributed code as a positionless event of that code", () => {
    const halted = record({ diagnostics: [diagnostic("REACTIVITY_HALTED", 0)] });
    expect(runtimeDiff(halted, record({ unattributed: ["REACTIVITY_HALTED"] }))).toEqual([]);
    expect(runtimeDiff(record(), record({ unattributed: ["SETTLED_CLEANUP_UNOWNED"] }))).toEqual([{ kind: "chrome-only", detail: "SETTLED_CLEANUP_UNOWNED@-" }]);
  });

  it("compares the sites that ran, the witness texts, the failure phase and the unattributed codes", () => {
    const diffs = runtimeDiff(record(), record({
      hits: { s1: 1, s2: 2 },
      witnesses: [{ siteId: "s1", witness: { selector: "#w1", text: "1" }, actual: "0" }],
      failure: { phase: "halted" },
      unattributed: ["UNCOMMITTED_INPUT_OVERWRITTEN"],
    }));
    expect(diffs.map((d) => d.kind).sort()).toEqual(["chrome-only", "failure", "hits", "witness"]);
    expect(diffs.find((d) => d.kind === "hits").detail).toBe("happy-only [] chrome-only [s2]");
  });

  it("reads no hit difference where one side has no ledger", () => {
    expect(runtimeDiff(record({ hits: null }), record())).toEqual([]);
  });

  it("reports a missing record", () => {
    expect(runtimeDiff(undefined, record())).toEqual([{ kind: "missing", detail: "no Happy DOM record" }]);
  });
});

describe("sampleStrata", () => {
  const row = (id, extra) => ({ id, code: "C", context: "body", binding: "direct", layer: "none", outcome: "agree", lines: 10, index: 0, ...extra });

  it("takes up to N programs per stratum, the same ones every time, and all with Infinity", () => {
    const rows = [row("a", { binding: "alias" }), row("b", { layer: "iife" }), row("c"), row("d", { source: "prop" })];
    const two = sampleStrata(rows, 2);
    expect(two).toHaveLength(3);
    expect(two.filter((r) => !r.source)).toHaveLength(2);
    expect(sampleStrata([...rows].reverse(), 2).map((r) => r.id)).toEqual(two.map((r) => r.id));
    expect(sampleStrata(rows, Infinity)).toHaveLength(4);
  });

  it("keys the default stratum on code, context, source, mount and outcome; the full key on every axis", () => {
    expect(stratumOf(row("a"), DEFAULT_DIFF_AXES)).toBe("code=C|context=body|source=-|mount=-|outcome=agree");
    expect(stratumOf(row("a"))).toBe("code=C|context=body|variant=-|binding=direct|layer=none|mount=-|source=-|outcome=agree");
    expect(sampleStrata([row("a"), row("b", { outcome: "recall-gap" })], 1, (r) => r.id !== "a").map((r) => r.id)).toEqual(["b"]);
    expect(sampleStrata([row("a"), row("b", { binding: "alias" })], 1, () => true, ["code", "binding"])).toHaveLength(2);
  });

  it("reads --runtime-diff-axes and refuses an unknown axis", () => {
    expect(parseAxes(undefined)).toEqual(DEFAULT_DIFF_AXES);
    expect(parseAxes("code,binding")).toEqual(["code", "binding"]);
    expect(() => parseAxes("code,colour")).toThrow();
  });
});

describe("parsePerStratum and summarize", () => {
  it("reads a count or all, and refuses anything else", () => {
    expect(parsePerStratum(undefined)).toBe(0);
    expect(parsePerStratum("3")).toBe(3);
    expect(parsePerStratum("all")).toBe(Infinity);
    expect(() => parsePerStratum("0")).toThrow();
    expect(() => parsePerStratum("x")).toThrow();
  });

  it("counts the programs per kind once", () => {
    const summary = summarize([{ diffs: [] }, { diffs: [{ kind: "hits" }, { kind: "hits" }] }, { diffs: [{ kind: "chrome-only" }] }]);
    expect(summary).toEqual({ compared: 3, same: 1, differ: 2, behaviour: 2, attributionOnly: 0, byKind: { hits: 1, "chrome-only": 1 } });
  });
});
