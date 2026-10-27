import { describe, expect, it } from "vitest";
import { VERDICTS } from "./classify.mjs";
import {
  applyLedger, coarseKey, confirmFalseClean, docTargets, frameKind, groupFalseCleans, isConfirmed, ledgerProblems, legalTwin, loadLedger,
  primChain, rankRecallGaps, slug, stackShape, staleOpen, stillShowsFalseClean, twinIsClean,
} from "./falseclean.mjs";
import { generateProgram } from "./generate.mjs";
import { buildCase } from "./emit.mjs";
import { component, decl, fn, iife, own, program as makeProgram, prim, ret, probe, signal } from "./model.mjs";
import { printProgram } from "./print.mjs";

const rank = (kind) => VERDICTS.indexOf(kind);

// A component whose body is `const x = <host>(() => { return <site> })`, hosts listed outermost first.
const hostProgram = (hosts, site = { k: "own", op: "cleanup", site: true }) => {
  let inner = site;
  for (const name of [...hosts].reverse()) inner = { k: "prim", name, fn: { k: "fn", form: "arrow", async: false, params: [], body: [{ k: "return", e: inner }] } };
  const program = { k: "program", components: [{ k: "component", name: "C1", signals: [], body: [{ k: "decl", name: "x", e: inner }], view: [] }] };
  const sitePath = ["components", 0, "body", 0, "e", ...hosts.flatMap(() => ["fn", "body", 0, "e"])];
  return { program, sitePath };
};

describe("the primitive chain", () => {
  it("lists the hosts above the site, root first, and leaves out the owner call itself", () => {
    const { program, sitePath } = hostProgram(["root", "untrack"]);
    expect(primChain(program, sitePath)).toBe("root>untrack");
    expect(primChain(hostProgram(["memo"]).program, hostProgram(["memo"]).sitePath)).toBe("memo");
  });
  it("is - for a site in the component body", () => {
    const program = { k: "program", components: [{ k: "component", body: [{ k: "expr", e: { k: "own", op: "cleanup", site: true } }], view: [] }] };
    expect(primChain(program, ["components", 0, "body", 0, "e"])).toBe("-");
  });
  it("tells runWithOwner(null) from runWithOwner(owner)", () => {
    const program = { k: "program", components: [{ k: "component", body: [{ k: "expr", e: { k: "prim", name: "owner", ownerArg: "owner", fn: { k: "fn", body: [{ k: "expr", e: { k: "own", op: "cleanup", site: true } }] } } }], view: [] }] };
    const path = ["components", 0, "body", 0, "e", "fn", "body", 0, "e"];
    expect(primChain(program, path)).toBe("owner:owner");
    program.components[0].body[0].e.ownerArg = undefined;
    expect(primChain(program, path)).toBe("owner:null");
  });
});

describe("the stack shape", () => {
  const table = [
    { line: 3, endLine: 9, path: [], label: "prim:memo" },
    { line: 4, endLine: 6, path: [], label: "fn:arrow" },
    { line: 12, endLine: 12, path: [], label: "button" },
    { line: 14, endLine: 14, path: [], label: "child" },
  ];
  it("keeps the roles that enter code (primitive, handler, JSX), innermost first, and collapses runs", () => {
    expect(stackShape(table, [5, 4, 3, 12])).toBe("prim:memo<handler");
    expect(stackShape(table, [14, 5])).toBe("jsx<prim:memo");
  });
  it("ignores lines the table does not know and wrappers; - when nothing is left", () => {
    expect(stackShape(table, [99, 98])).toBe("-");
    expect(stackShape([{ line: 1, endLine: 1, path: [], label: "fn:arrow" }], [1])).toBe("-");
    expect(stackShape(table, undefined)).toBe("-");
  });
  it("names frame roles", () => {
    expect(frameKind("prim:effect")).toBe("prim:effect");
    expect(frameKind("button")).toBe("handler");
    expect(frameKind("call")).toBeNull();
    expect(frameKind(undefined)).toBeNull();
  });
});

describe("the coarse key", () => {
  const diag = (line, frames) => ({ code: "NO_OWNER_CLEANUP", line, frames });
  it("is a code, a chain and a shape for generated owner programs", () => {
    const a = buildCase(generateProgram(2654435762, { profile: "owner-cleanup" }));
    const b = buildCase(generateProgram(1013904227, { profile: "owner-cleanup" }));
    for (const built of [a, b]) {
      const site = built.sites[0];
      const program = JSON.parse(built.files["program.json"]);
      const key = coarseKey({ diagnostic: diag(site.line, [site.line]), program, lineTable: built.lineTable });
      expect(key).toMatch(/^NO_OWNER_CLEANUP\|[^|]+\|[^|]+$/);
    }
  });
  it("is unchanged by wrapper layers around the same cause (the fine key keeps them apart) and changed by the host", () => {
    const keyOf = (host, wrap) => {
      const site = own("cleanup", { site: true, witness: null });
      const body = [decl("m1", prim(host, fn([ret(wrap ? iife(fn([ret(site)])) : site)])))];
      const program = makeProgram({ components: [component("C1", [signal("w1", 0, "probe")], body, [probe("w1")])] });
      const printed = printProgram(program);
      const line = printed.sites[0].line;
      return coarseKey({ diagnostic: { code: "NO_OWNER_CLEANUP", line, frames: [line] }, program, lineTable: printed.lineTable });
    };
    expect(keyOf("memo", false)).toBe("NO_OWNER_CLEANUP|memo|prim:memo");
    expect(keyOf("memo", true)).toBe(keyOf("memo", false));
    expect(keyOf("untrack", false)).not.toBe(keyOf("memo", false));
  });
  it("has no chain and no shape for a diagnostic no line could be given for", () => {
    expect(coarseKey({ diagnostic: { code: "NO_OWNER_CLEANUP", line: null }, program: {}, lineTable: [] })).toBe("NO_OWNER_CLEANUP|-|unplaced");
  });
  it("reads the chain of the owner call on the line, not of a callback argument nested deeper on the same line", () => {
    const program = { k: "program", components: [{ k: "component", body: [{ k: "expr", e: { k: "prim", name: "untrack", fn: { k: "fn", body: [{ k: "expr", e: { k: "own", op: "cleanup", site: true, fn: { k: "fn", body: [] } } }] } } }], view: [] }] };
    const own = ["components", 0, "body", 0, "e", "fn", "body", 0, "e"];
    const table = [
      { line: 5, endLine: 7, path: ["components", 0, "body", 0, "e"], label: "prim:untrack" },
      { line: 6, endLine: 6, path: own, label: "own:cleanup" },
      { line: 6, endLine: 6, path: [...own, "fn"], label: "fn:arrow" },
    ];
    expect(coarseKey({ diagnostic: { code: "NO_OWNER_CLEANUP", line: 6, frames: [6, 5] }, program, lineTable: table })).toBe("NO_OWNER_CLEANUP|untrack|prim:untrack");
  });
});

describe("the ledger", () => {
  const targets = { anchors: new Map([["open-work.md", new Set(["2-decisions-not-to-build"])]]), adrs: new Set(["0110"]) };
  const entry = (over = {}) => ({ code: "NO_OWNER_CLEANUP", coarseKey: "NO_OWNER_CLEANUP|untrack|prim:untrack<handler", ref: "open-work.md#2-decisions-not-to-build", reason: "accepted", ...over });
  it("accepts an entry that cites an anchor or an ADR that exists", () => {
    expect(ledgerProblems([entry(), entry({ coarseKey: "NO_OWNER_CLEANUP|memo|prim:memo", ref: "ADR 0110" })], targets)).toEqual([]);
  });
  it("refuses wildcards, unknown or non-defect codes, missing references, duplicates and bad shapes", () => {
    const joined = (entries) => ledgerProblems(entries, targets).join("; ");
    expect(joined([entry({ coarseKey: "NO_OWNER_CLEANUP|*|*" })])).toMatch(/wildcard/);
    expect(joined([entry({ code: "REACTIVITY_HALTED", coarseKey: "REACTIVITY_HALTED|-|-" })])).toMatch(/not a defect code/);
    expect(joined([entry({ code: "NOT_A_CODE", coarseKey: "NOT_A_CODE|-|-" })])).toMatch(/not a defect code/);
    expect(joined([entry({ ref: "open-work.md#no-such-anchor" })])).toMatch(/no anchor/);
    expect(joined([entry({ ref: "ADR 9999" })])).toMatch(/no ADR 9999/);
    expect(joined([entry({ ref: "see the issue tracker" })])).toMatch(/neither/);
    expect(joined([entry({ reason: " " })])).toMatch(/reason is missing/);
    expect(joined([entry({ coarseKey: "STRICT_READ_UNTRACKED|-|-" })])).toMatch(/does not start with its code/);
    expect(joined([entry(), entry()])).toMatch(/duplicate/);
    expect(joined({})).toMatch(/not an array/);
  });
  it("reads real anchors and ADRs from the docs, and the shipped ledger is valid", () => {
    const real = docTargets();
    expect(real.anchors.get("open-work.md").has("2-decisions-not-to-build")).toBe(true);
    expect(real.adrs.has("0110")).toBe(true);
    expect(slug("2. Decisions not to build")).toBe("2-decisions-not-to-build");
    expect(() => loadLedger()).not.toThrow();
  });
  const falseCleanValue = (code = "NO_OWNER_CLEANUP") => {
    const built = buildCase(generateProgram(2654435762, { profile: "owner-cleanup" }));
    const line = built.sites[0].line;
    const program = JSON.parse(built.files["program.json"]);
    return { program, built, classification: { verdict: "candidate-false-clean", reasons: ["r"], items: [{ kind: "candidate-false-clean", reason: "r", diagnostic: { code, line, frames: [line], severity: "warn" } }] } };
  };
  it("turns a false clean it lists into known-open, by exact code and key, and reports what fired", () => {
    const value = falseCleanValue();
    const key = coarseKey({ diagnostic: value.classification.items[0].diagnostic, program: value.program, lineTable: value.built.lineTable });
    const fired = applyLedger(value, [entry({ coarseKey: key })], rank);
    expect(fired).toHaveLength(1);
    expect(value.classification.verdict).toBe("known-open");
    expect(value.classification.items[0]).toMatchObject({ kind: "known-open", ref: "open-work.md#2-decisions-not-to-build", coarseKey: key });
  });
  it("leaves a false clean it does not list, and a near key does not match", () => {
    const value = falseCleanValue();
    const fired = applyLedger(value, [entry({ coarseKey: "NO_OWNER_CLEANUP|untrack|prim:untrack" })], rank);
    expect(fired).toEqual([]);
    expect(value.classification.verdict).toBe("candidate-false-clean");
    expect(value.classification.items[0].coarseKey).toMatch(/^NO_OWNER_CLEANUP\|/);
  });
  it("names the entries that never fired", () => {
    const ledger = [entry(), entry({ coarseKey: "NO_OWNER_CLEANUP|memo|prim:memo" })];
    expect(staleOpen(ledger, [ledger[0]])).toEqual([ledger[1]]);
  });
});

describe("groups", () => {
  const item = (code, severity) => ({ diagnostic: { code, severity } });
  it("groups by key, represents a group by its smallest program, and ranks errors before warnings", () => {
    const groups = groupFalseCleans([
      { id: "a", key: "NO_OWNER_CLEANUP|memo|-", item: item("NO_OWNER_CLEANUP", "warn"), size: 20 },
      { id: "b", key: "NO_OWNER_CLEANUP|memo|-", item: item("NO_OWNER_CLEANUP", "warn"), size: 12 },
      { id: "c", key: "CLEANUP_IN_FORBIDDEN_SCOPE|settled|-", item: item("CLEANUP_IN_FORBIDDEN_SCOPE", "error"), size: 30 },
    ]);
    expect(groups.map((g) => [g.coarseKey, g.count, g.representative.id])).toEqual([["CLEANUP_IN_FORBIDDEN_SCOPE|settled|-", 1, "c"], ["NO_OWNER_CLEANUP|memo|-", 2, "b"]]);
  });
  it("ranks recall gaps by key and rule", () => {
    const gap = (rule) => ({ key: "STRICT_READ_UNTRACKED|root|-", item: { diagnostic: { rule } } });
    expect(rankRecallGaps([gap("a"), gap("b"), gap("a")])).toEqual([{ signature: "STRICT_READ_UNTRACKED|root|-|a", count: 2 }, { signature: "STRICT_READ_UNTRACKED|root|-|b", count: 1 }]);
  });
});

describe("Chrome's confirmation", () => {
  const item = (line = 6) => ({ kind: "candidate-false-clean", diagnostic: { code: "NO_OWNER_CLEANUP", line } });
  const fc = (line) => ({ kind: "candidate-false-clean", diagnostic: { code: "NO_OWNER_CLEANUP", line } });
  const record = (over = {}) => ({ runtime: { failure: null, diagnostics: [], errors: [], unattributed: [], ...over.runtime }, checker: { ok: true, findings: [], perturbed: [], ...over.checker } });
  const status = (candidate, judgedItems, rec = record()) => confirmFalseClean({ item: candidate, judged: { items: judgedItems, reasons: ["r"] }, record: rec }).status;
  it("confirms the code at the same innermost line", () => {
    expect(status(item(6), [fc(6)])).toBe("confirmed");
  });
  it("does not confirm a different line, a covering finding, or silence", () => {
    expect(status(item(6), [fc(9)])).toBe("line-differs");
    expect(status(item(6), [{ kind: "recall-gap", diagnostic: { code: "NO_OWNER_CLEANUP", line: 6 } }], record({ runtime: { diagnostics: [{ code: "NO_OWNER_CLEANUP", positions: [{ file: "App.tsx", line: 6 }] }] } }))).toBe("covered-in-chrome");
    // A violation covers the diagnostic in Chrome: an agree item carries no diagnostic, the run's own events say Chrome reported it.
    expect(status(item(6), [{ kind: "agree", finding: {} }], record({ runtime: { diagnostics: [{ code: "NO_OWNER_CLEANUP", positions: [{ file: "App.tsx", line: 6 }] }] } }))).toBe("covered-in-chrome");
    expect(status(item(6), [])).toBe("refuted-by-chrome");
  });
  it("counts a diagnostic Chrome could not place as Solid speaking, for a false clean", () => {
    expect(status(item(6), [], record({ runtime: { unattributed: ["NO_OWNER_CLEANUP"] } }))).toBe("confirmed-unplaced");
    // ... unless a finding that could claim it exists somewhere in the file.
    expect(status(item(6), [], record({ runtime: { unattributed: ["NO_OWNER_CLEANUP"] }, checker: { findings: [{ rule: "missing-owner", kind: "uncertifiable", line: 2, endLine: 2 }] } }))).toBe("refuted-by-chrome");
    expect(status(item(null), [{ kind: "candidate-false-clean", diagnostic: { code: "NO_OWNER_CLEANUP", line: null } }])).toBe("confirmed-unplaced");
    expect(isConfirmed("confirmed-unplaced") && isConfirmed("confirmed") && !isConfirmed("line-differs")).toBe(true);
  });
  it("is inconclusive when the Chrome run failed or its checker was perturbed", () => {
    expect(status(item(6), [fc(6)], record({ runtime: { failure: { phase: "x", message: "m" } } }))).toBe("chrome-inconclusive");
    expect(status(item(6), [fc(6)], record({ checker: { perturbed: ["rule differs"] } }))).toBe("chrome-inconclusive");
    expect(confirmFalseClean({ item: item(6), judged: null, record: null }).status).toBe("chrome-inconclusive");
  });
});

describe("the shrink's predicate and control", () => {
  const built = buildCase(generateProgram(2654435762, { profile: "owner-cleanup" }));
  const program = JSON.parse(built.files["program.json"]);
  it("builds a legal twin: the same tree with its payload sites replaced by literals", () => {
    const twin = legalTwin(program);
    expect(twin).not.toBeNull();
    const sites = JSON.stringify(twin);
    expect(sites).not.toContain('"site":true');
    expect(JSON.stringify(program)).toContain('"site":true');
    expect(buildCase(twin).sites).toEqual([]);
    expect(legalTwin(twin)).toBeNull();
  });
  it("accepts a candidate only with the same code and the same coarse key", () => {
    const line = built.sites[0].line;
    const value = { program, built, classification: { items: [{ kind: "candidate-false-clean", diagnostic: { code: "NO_OWNER_CLEANUP", line, frames: [line] } }] } };
    const key = coarseKey({ diagnostic: value.classification.items[0].diagnostic, program, lineTable: built.lineTable });
    expect(stillShowsFalseClean(value, { code: "NO_OWNER_CLEANUP", key })).toBe(true);
    expect(stillShowsFalseClean(value, { code: "NO_OWNER_CLEANUP", key: `${key}x` })).toBe(false);
    expect(stillShowsFalseClean(value, { code: "NO_OWNER_EFFECT", key })).toBe(false);
  });
  it("holds the control only when the twin ran clean of the code", () => {
    const ran = (diagnostics) => ({ typecheck: { ok: true }, checker: { ok: true, findings: [] }, runtime: { diagnostics, errors: [] } });
    const diag = { code: "NO_OWNER_CLEANUP", message: "m", positions: [{ file: "App.tsx", line: 4 }] };
    expect(twinIsClean(ran([]), "NO_OWNER_CLEANUP")).toBe(true);
    expect(twinIsClean(ran([diag]), "NO_OWNER_CLEANUP")).toBe(false);
    expect(twinIsClean(ran([diag]), "NO_OWNER_EFFECT")).toBe(true);
    expect(twinIsClean({ typecheck: { ok: false } }, "NO_OWNER_CLEANUP")).toBe(false);
    expect(twinIsClean(undefined, "NO_OWNER_CLEANUP")).toBe(false);
  });
});
