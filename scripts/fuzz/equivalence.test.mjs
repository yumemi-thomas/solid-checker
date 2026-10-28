import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { classifyDivergence, compareVariant, groupDivergences, judgeDivergence, planVariants, runEquivalence, selectKinds, siteMap, siteSummary, subsetChooser, sameRun } from "./equivalence.mjs";
import { generateProgram } from "./generate.mjs";
import { REWRITE_KINDS, rewriteProgram } from "./rewrite.mjs";

const v = (verdict, ...rules) => ({ verdict, rules });

describe("classifyDivergence", () => {
  it("is silent for the same verdict and rules", () => {
    expect(classifyDivergence(v("violation", "a"), v("violation", "a"))).toBeNull();
    expect(classifyDivergence(v("none"), v("none"))).toBeNull();
    expect(classifyDivergence(v("uncertifiable", "a"), v("uncertifiable", "a"))).toBeNull();
  });
  it("puts a violation against clean, and two different rules, in the high bucket", () => {
    expect(classifyDivergence(v("violation", "a"), v("none"))).toMatchObject({ bucket: "high", cls: "violation-vs-clean", direction: "original" });
    expect(classifyDivergence(v("none"), v("violation", "a"))).toMatchObject({ bucket: "high", direction: "variant" });
    expect(classifyDivergence(v("violation", "a"), v("violation", "b"))).toMatchObject({ bucket: "high", cls: "rule-code" });
  });
  it("puts uncertifiable against clean in its own middle bucket and names the clean side", () => {
    expect(classifyDivergence(v("uncertifiable", "a"), v("none"))).toMatchObject({ bucket: "certainty-divergence", cleanSide: "variant" });
    expect(classifyDivergence(v("none"), v("uncertifiable", "a"))).toMatchObject({ bucket: "certainty-divergence", cleanSide: "original" });
  });
  it("keeps a violation against uncertifiable low", () => {
    expect(classifyDivergence(v("violation", "a"), v("uncertifiable", "a"))).toMatchObject({ bucket: "low", cls: "violation-vs-uncertifiable" });
    expect(classifyDivergence(v("uncertifiable", "a"), v("uncertifiable", "b"))).toMatchObject({ bucket: "low", cls: "uncertifiable-rule" });
  });
});

describe("siteSummary", () => {
  it("names the violating rules of a violation, and every rule of an uncertifiable site", () => {
    const findings = [{ rule: "a", kind: "violation" }, { rule: "b", kind: "uncertifiable" }];
    expect(siteSummary({ verdict: "violation", findings })).toEqual(v("violation", "a"));
    expect(siteSummary({ verdict: "uncertifiable", findings: [findings[1]] })).toEqual(v("uncertifiable", "b"));
    expect(siteSummary({ verdict: "none", findings: [] })).toEqual(v("none"));
  });
});

describe("compareVariant", () => {
  const verdict = (siteId, kind, ...rules) => ({ siteId, line: 1, verdict: kind, findings: rules.map((rule) => ({ rule, kind })) });
  it("compares by the site map, not by site id", () => {
    const map = new Map([["s1", "s2"], ["s2", "s1"]]);
    const { divergences, lost } = compareVariant({
      originalSites: [{ id: "s1" }, { id: "s2" }], originalVerdicts: [verdict("s1", "violation", "a"), verdict("s2", "none")],
      variantSites: [], variantVerdicts: [verdict("s1", "none"), verdict("s2", "violation", "a")], map,
    });
    expect(divergences).toEqual([]);
    expect(lost).toEqual([]);
  });
  it("reports a divergence and a lost site", () => {
    const { divergences, lost } = compareVariant({
      originalSites: [{ id: "s1" }, { id: "s2" }], originalVerdicts: [verdict("s1", "violation", "a"), verdict("s2", "none")],
      variantSites: [], variantVerdicts: [verdict("t1", "none")], map: new Map([["t1", "s1"]]),
    });
    expect(divergences).toHaveLength(1);
    expect(divergences[0]).toMatchObject({ siteId: "s1", bucket: "high" });
    expect(lost).toEqual(["s2"]);
  });
});

describe("planning", () => {
  const program = generateProgram(7, { profile: "in-place" });
  it("is deterministic, cycles through applicable kinds and respects the allow list", () => {
    expect(planVariants(program, 5)).toEqual(planVariants(program, 5));
    const plan = planVariants(program, 40);
    expect(new Set(plan.map((p) => p.kind)).size).toBeGreaterThan(3);
    expect(planVariants(program, 6, { kinds: ["rename"] }).every((p) => p.kind === "rename")).toBe(true);
    expect(planVariants(program, 3, { kinds: [] })).toEqual([]);
  });
  it("chooses a non-empty subset, deterministically", () => {
    const all = [1, 2, 3, 4, 5];
    expect(subsetChooser(5)(all)).toEqual(subsetChooser(5)(all));
    for (let seed = 1; seed < 40; seed += 1) expect(subsetChooser(seed)(all).length).toBeGreaterThan(0);
  });
  it("selects kinds and refuses an unknown one", () => {
    expect(selectKinds({})).toEqual(REWRITE_KINDS);
    expect(selectKinds({ rewrites: "rename,iife" })).toEqual(["rename", "iife"]);
    expect(selectKinds({ excludeRewrites: "rename" })).not.toContain("rename");
    expect(() => selectKinds({ rewrites: "nope" })).toThrow(/unknown rewrite nope/);
  });
});

describe("runEquivalence", () => {
  const entry = (seed) => { const program = generateProgram(seed, { profile: "in-place" }); return { program, built: buildCase(program, { id: `fuzz-${seed}` }) }; };
  const config = () => ({ install: mkdtempSync(join(tmpdir(), "fuzz-eq-")), concurrency: 2 });
  const ok = async () => ({ ok: true, errors: [] });

  it("is silent when the checker answers the same on every form", async () => {
    const result = await runEquivalence([entry(1), entry(2)], config(), { n: 3 }, { typecheck: ok, runChecker: async () => ({ ok: true, findings: [] }) });
    expect(result.divergences).toEqual([]);
    expect(result.programs.every((row) => row.status === "ok")).toBe(true);
  });

  it("finds a divergence when the checker is told something only on the original text, and maps it back to the original site", async () => {
    const e = entry(3);
    const site = e.built.sites[0];
    const original = e.built.text;
    const result = await runEquivalence([e], config(), { n: 4 }, {
      typecheck: ok,
      // A checker that proves a violation at the first site's line only in the text it was first shown.
      runChecker: async (dir) => {
        const { readFileSync } = await import("node:fs");
        const text = readFileSync(join(dir, "App.tsx"), "utf8");
        const line = text.split("\n").findIndex((l) => l.includes(site.kind === "write" ? "set" : "()")) + 1;
        return { ok: true, findings: text === original ? [{ file: "App.tsx", rule: "r", id: "SC", kind: "violation", line: site.line, endLine: site.line }] : [{ file: "App.tsx", rule: "r", id: "SC", kind: "uncertifiable", line, endLine: line }] };
      },
    });
    expect(result.divergences.length).toBeGreaterThan(0);
    for (const found of result.divergences) {
      expect(found.kind).toBe("equivalence-divergence");
      expect(found.siteId).toBe(site.id);
      expect(found.original.verdict).toBe("violation");
      expect(found.signature.startsWith(`${found.rewrite}|`)).toBe(true);
    }
    const groups = groupDivergences(result.divergences);
    expect(groups.high.length + groups["certainty-divergence"].length + groups.low.length).toBeGreaterThan(0);
  });

  it("drops a variant that does not type-check and counts it, and a program the checker failed on", async () => {
    let calls = 0;
    const result = await runEquivalence([entry(4)], config(), { n: 3 }, {
      typecheck: async () => { calls += 1; return calls === 1 ? { ok: true, errors: [] } : { ok: false, errors: ["error TS2322"] }; },
      runChecker: async () => ({ ok: true, findings: [] }),
    });
    expect(result.variants["type-error"]).toBeGreaterThan(0);
    const failed = await runEquivalence([entry(5)], config(), { n: 2 }, { typecheck: ok, runChecker: async () => ({ ok: false, findings: [], error: "boom" }) });
    expect(failed.programs[0].status).toBe("checker-failed");
  });
});

describe("the site map", () => {
  it("follows the tag through a rewrite that moves lines", () => {
    const program = generateProgram(8, { profile: "in-place" });
    const rewritten = rewriteProgram(program, "dead-decl");
    const built = buildCase(rewritten.program, { id: "x" });
    const map = siteMap(rewritten.program, built.sites);
    expect([...map.values()].sort()).toEqual(buildCase(program, { id: "y" }).sites.map((site) => site.id).sort());
  });
});

describe("judging a divergence against Chrome's two runs", () => {
  const record = (codes, over = {}) => ({ runtime: { diagnostics: codes.map((code) => ({ code, positions: [{ file: "App.tsx", line: 7 }] })), errors: [], witnesses: [{ siteId: "s1", actual: "5", reached: true }], hits: {}, ...over } });
  const built = { sites: [{ id: "s1", line: 7, hit: 1 }], plan: { clicks: [] } };
  const divergence = (extra) => ({ siteId: "s1", bucket: "high", original: v("violation", "no-write-in-pure-computation"), variant: v("none"), originalFindings: [{ kind: "violation", rule: "no-write-in-pure-computation", line: 7, endLine: 7 }], variantFindings: [], ...extra });
  const map = new Map([["s1", "s1"]]);

  it("confirms a violation when Chrome reports the code its rule claims", () => {
    const judged = judgeDivergence(divergence(), built, built, record(["REACTIVE_WRITE_IN_OWNED_SCOPE"]), record(["REACTIVE_WRITE_IN_OWNED_SCOPE"]), map);
    expect(judged.status).toBe("violation-confirmed");
  });
  it("calls the violation side a false positive when the site ran in silence on both runs", () => {
    expect(judgeDivergence(divergence(), built, built, record([]), record([]), map)).toMatchObject({ status: "violation-contradicted", sides: ["original"] });
  });
  it("still confirms a violation from the diagnostic a run reached before the driver failed", () => {
    const halted = (codes) => record(codes, { failure: { phase: "execution", message: "locator.click: Timeout" } });
    expect(judgeDivergence(divergence(), built, built, halted(["REACTIVE_WRITE_IN_OWNED_SCOPE"]), halted(["REACTIVE_WRITE_IN_OWNED_SCOPE"]), map).status).toBe("violation-confirmed");
    // Two cut-short runs that differ prove nothing about the rewrite.
    expect(judgeDivergence(divergence(), built, built, halted(["REACTIVE_WRITE_IN_OWNED_SCOPE"]), halted([]), map).status).toBe("chrome-inconclusive");
  });
  it("never refutes from a partial run: no diagnostic is never a false positive, an unsound rewrite or a silent site", () => {
    const halted = (codes) => record(codes, { failure: { phase: "execution", message: "locator.click: Timeout" } });
    const certainty = divergence({ bucket: "certainty-divergence", original: v("uncertifiable", "no-write-in-pure-computation"), originalFindings: [{ kind: "uncertifiable", rule: "x", line: 7, endLine: 7 }] });
    expect(judgeDivergence(divergence(), built, built, halted([]), halted([]), map).status).toBe("inconclusive");
    expect(judgeDivergence(divergence(), built, built, halted([]), halted([]), map).status).not.toBe("violation-contradicted");
    expect(judgeDivergence(divergence(), built, built, halted(["REACTIVE_WRITE_IN_OWNED_SCOPE"]), halted([]), map).status).not.toBe("unsound-rewrite");
    expect(judgeDivergence(certainty, built, built, halted([]), halted([]), map)).toMatchObject({ status: "inconclusive", why: "partial-run" });
    // The other direction: a diagnostic reached before the failure still shows the defect.
    expect(judgeDivergence(certainty, built, built, halted(["REACTIVE_WRITE_IN_OWNED_SCOPE"]), halted(["REACTIVE_WRITE_IN_OWNED_SCOPE"]), map).status).toBe("defect-at-site");
  });
  it("is inconclusive when the run was disturbed", () => {
    const disturbed = record([], { failure: { phase: "execution", message: "x" } });
    expect(judgeDivergence(divergence(), built, built, disturbed, disturbed, map).status).toBe("inconclusive");
  });
  it("calls a rewrite unsound when the two runs differ", () => {
    expect(judgeDivergence(divergence(), built, built, record(["REACTIVE_WRITE_IN_OWNED_SCOPE"]), record([]), map).status).toBe("unsound-rewrite");
    expect(sameRun(record([]), record([]), map).same).toBe(true);
  });
  it("reports whether Chrome sees a defect for a certainty divergence", () => {
    const certainty = divergence({ bucket: "certainty-divergence", original: v("uncertifiable", "no-write-in-pure-computation"), originalFindings: [{ kind: "uncertifiable", rule: "x", line: 7, endLine: 7 }] });
    expect(judgeDivergence(certainty, built, built, record(["REACTIVE_WRITE_IN_OWNED_SCOPE"]), record(["REACTIVE_WRITE_IN_OWNED_SCOPE"]), map).status).toBe("defect-at-site");
    expect(judgeDivergence(certainty, built, built, record([]), record([]), map).status).toBe("silent-at-site");
  });
});
