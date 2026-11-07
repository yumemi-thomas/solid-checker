import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { LAYERS, RECALL_LAYERS } from "./misuse.mjs";
import { ALLOWED, TWIN_CODES, TWIN_LAYERS, allowedBy, answersOf, groupKey, judgeGroup, parse, rejudge, twinLayer, twinPrograms } from "./twins.mjs";

const member = (layer, answers) => ({ id: `p-${layer}`, layer, answers });

describe("the twin layers", () => {
  it("classifies every layer the grids draw, and stops on one nobody classified", () => {
    for (const layer of [...LAYERS, ...RECALL_LAYERS]) expect(TWIN_LAYERS[layer]).toBeTruthy();
    expect(() => twinLayer("handoff-timeout")).toThrow(/not classified/);
    expect(ALLOWED).toEqual([]);
  });
});

describe("a program's answers", () => {
  it("is per defect code: a violation of a rule that claims it anywhere, else a declining finding, else none", () => {
    const answers = answersOf([
      { rule: "strict-read-untracked", kind: "uncertifiable", line: 3 }, { rule: "strict-read-untracked", kind: "violation", line: 9 },
      { rule: "no-write-in-pure-computation", kind: "uncertifiable", line: 4 }, { rule: "prefer-show", kind: "violation", line: 2 },
      { rule: "missing-owner", kind: "violation", line: 5 },
    ]);
    expect(answers.STRICT_READ_UNTRACKED).toBe("violation");
    expect(answers.REACTIVE_WRITE_IN_OWNED_SCOPE).toBe("uncertifiable");
    expect(answers.ACTION_CALLED_IN_OWNED_SCOPE).toBe("none");
    // One rule claims several codes: its violation answers each.
    expect([answers.NO_OWNER_EFFECT, answers.NO_OWNER_BOUNDARY]).toEqual(["violation", "violation"]);
    // An advisory code is not judged: its silence is no miss.
    expect(TWIN_CODES).not.toContain("ASYNC_OUTSIDE_LOADING_BOUNDARY");
    expect(answers.ASYNC_OUTSIDE_LOADING_BOUNDARY).toBeUndefined();
    // A withholding rule's finding on the site's lines declines every code; elsewhere it declines none (review 4c).
    const withheldFindings = [{ rule: "reactive-dispatch-unresolved", kind: "uncertifiable", line: 8 }, { rule: "missing-owner", kind: "violation", line: 5 }];
    const withheld = answersOf(withheldFindings, [8]);
    expect([withheld.PRIMITIVE_IN_FORBIDDEN_SCOPE, withheld.NO_OWNER_BOUNDARY]).toEqual(["uncertifiable", "violation"]);
    const elsewhere = answersOf(withheldFindings, [3]);
    expect([elsewhere.PRIMITIVE_IN_FORBIDDEN_SCOPE, elsewhere.NO_OWNER_BOUNDARY]).toEqual(["none", "violation"]);
  });
});

describe("a group's judgement", () => {
  const none = answersOf([]);
  const at = (code, kind) => ({ ...none, [code]: kind });
  const sru = (kind) => at("STRICT_READ_UNTRACKED", kind);

  it("flags a legal twin that proves what a twin of it declines (full-7's effect-error-arm closure)", () => {
    const flags = judgeGroup({ kind: "legal", code: "STRICT_READ_UNTRACKED" }, [member("none", sru("uncertifiable")), member("closure", sru("violation")), member("iife", sru("uncertifiable"))]);
    expect(flags).toEqual([{ code: "STRICT_READ_UNTRACKED", codes: ["STRICT_READ_UNTRACKED"], violating: [["closure", "p-closure", "violation"]], contradicting: [["none", "p-none", "uncertifiable"], ["iife", "p-iife", "uncertifiable"]] }]);
  });

  it("flags a violation beside a silent twin in either grid (full-7's reaction helper)", () => {
    const write = (kind) => at("REACTIVE_WRITE_IN_OWNED_SCOPE", kind);
    expect(judgeGroup({ kind: "legal", code: "REACTIVE_WRITE_IN_OWNED_SCOPE" }, [member("none", write("none")), member("helper", write("violation"))])).toHaveLength(1);
    // A misuse twin the checker calls clean where another is a proven defect: a false clean, or the proof is wrong.
    expect(judgeGroup({ kind: "misuse", code: "REACTIVE_WRITE_IN_OWNED_SCOPE" }, [member("none", write("violation")), member("depth-17", write("none"))])[0].contradicting).toEqual([["depth-17", "p-depth-17", "none"]]);
  });

  it("is one flag for the codes one rule's violation answers alike", () => {
    const owner = answersOf([{ rule: "missing-owner", kind: "violation", line: 4 }]);
    const flags = judgeGroup({ kind: "misuse", code: "NO_OWNER_BOUNDARY" }, [member("none", none), member("iife", owner)]);
    expect(flags).toHaveLength(1);
    expect(flags[0].codes).toEqual(TWIN_CODES.filter((code) => owner[code] === "violation"));
    expect(flags[0].codes.length).toBeGreaterThan(1);
  });

  it("holds the fail-closed direction: a misuse violation beside a declining twin, and declines beside silence", () => {
    expect(judgeGroup({ kind: "misuse", code: "STRICT_READ_UNTRACKED" }, [member("none", sru("violation")), member("depth-17", sru("uncertifiable"))])).toEqual([]);
    expect(judgeGroup({ kind: "legal", code: "STRICT_READ_UNTRACKED" }, [member("none", sru("none")), member("closure", sru("uncertifiable"))])).toEqual([]);
    expect(judgeGroup({ kind: "legal", code: "STRICT_READ_UNTRACKED" }, [member("none", sru("violation")), member("closure", sru("violation"))])).toEqual([]);
  });

  it("flags another code's violation beside a declining legal twin only when it is the cell's code", () => {
    // A legal program of one code may hold a real defect of another: the declining twin is then a recall gap, not a contradiction.
    expect(judgeGroup({ kind: "legal", code: "STORE_WRITE_OUTSIDE_SETTER" }, [member("none", sru("uncertifiable")), member("closure", sru("violation"))])).toEqual([]);
    expect(judgeGroup({ kind: "legal", code: "STORE_WRITE_OUTSIDE_SETTER" }, [member("none", none), member("closure", sru("violation"))])).toHaveLength(1);
  });

  it("holds a flag only by an allow-list entry naming every twin on its smaller side", () => {
    const flag = { code: "C", codes: ["C"], violating: [["depth-17", "a", "violation"]], contradicting: [["none", "b", "none"], ["closure", "c", "none"]] };
    const entry = { cell: "X", layers: ["depth-17"], reason: "why" };
    expect(allowedBy({ kind: "legal", code: "X", context: "c" }, flag, [entry])).toBe(entry);
    expect(allowedBy({ kind: "legal", code: "Y", context: "c" }, flag, [entry])).toBeNull();
    expect(allowedBy({ kind: "legal", code: "X", context: "c" }, { ...flag, violating: [["depth-17", "a", "violation"], ["iife", "d", "violation"]] }, [entry])).toBeNull();
  });
});

describe("the twin groups", () => {
  it("are the grids' programs keyed by everything but the layer, every group whole", () => {
    expect(groupKey("legal", "C", "memo", { variant: "signal", binding: "direct", layer: "iife" })).toBe(groupKey("legal", "C", "memo", { variant: "signal", binding: "direct", layer: "none" }));
    expect(groupKey("legal", "C", "memo", { variant: "signal", binding: "direct", layer: "iife", mount: "show" })).not.toBe(groupKey("legal", "C", "memo", { variant: "signal", binding: "direct", layer: "iife" }));
    const programs = twinPrograms({ seed: "1", codes: "FLUSH_IN_EFFECT_CALLBACK", grids: "legal" });
    const groups = new Map();
    for (const p of programs) groups.set(p.group, [...(groups.get(p.group) ?? []), p.shape.layer]);
    expect(programs.length).toBeGreaterThan(0);
    for (const layers of groups.values()) { expect(layers).toContain("none"); expect(new Set(layers).size).toBe(layers.length); }
    const sampled = twinPrograms({ seed: "1", codes: "FLUSH_IN_EFFECT_CALLBACK", grids: "legal", sample: "0.5", sampleSeed: "3" });
    expect(sampled.length).toBeLessThan(programs.length);
    for (const p of sampled) expect(sampled.filter((q) => q.group === p.group)).toHaveLength(groups.get(p.group).length);
    expect(twinPrograms({ seed: "1", codes: "FLUSH_IN_EFFECT_CALLBACK", grids: "legal", sample: "0.5", sampleSeed: "3" }).map((p) => p.group)).toEqual(sampled.map((p) => p.group));
  });

  it("judges a recorded run again from its findings (--from), without the checker", () => {
    const dir = mkdtempSync(join(tmpdir(), "fuzz-twins-"));
    const group = "legal|REACTIVE_WRITE_IN_OWNED_SCOPE|reaction|signal|direct|-|root|local";
    const row = (layer, findings) => ({ id: `p-${layer}`, kind: "legal", code: "REACTIVE_WRITE_IN_OWNED_SCOPE", context: "reaction", layer, group, outcome: "checked", findings });
    writeFileSync(join(dir, "twins.jsonl"), [row("none", []), row("helper", ["no-write-in-pure-computation/violation@13"]), { ...row("iife", []), outcome: "typed-away", findings: undefined }].map((r) => JSON.stringify(r)).join("\n"));
    expect(rejudge({ from: dir, out: join(dir, "again") })).toBe(1);
    const report = JSON.parse(readFileSync(join(dir, "again", "twins.json"), "utf8"));
    expect(report.summary).toMatchObject({ programs: 3, checked: 2, typedAway: 1, flaggedGroups: 1 });
    expect(report.flagged[0]).toMatchObject({ cell: "REACTIVE_WRITE_IN_OWNED_SCOPE", code: "REACTIVE_WRITE_IN_OWNED_SCOPE", rules: ["no-write-in-pure-computation"], violating: [["helper", "p-helper", "violation"]], contradicting: [["none", "p-none", "none"]] });
  });

  it("parses its options", () => {
    expect(parse(["--out", "/x", "--recall", "--codes", "A", "--sample", "0.1"])).toMatchObject({ out: "/x", recall: true, codes: "A", sample: "0.1" });
    expect(() => parse(["--codes", "A"])).toThrow(/--out/);
    expect(() => parse(["--out", "/x", "--sample", "2"])).toThrow(/fraction/);
  });
});
