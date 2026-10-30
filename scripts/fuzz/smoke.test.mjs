import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { GENERATOR_VERSION } from "./generate.mjs";
import { behaviourOf, compare, cover, featuresOf, keepIds, movedVerdicts } from "./smoke.mjs";

const row = (id, extra = {}) => ({ id, code: "C", context: "body", variant: "signal", binding: "direct", layer: "none", outcome: "agree", detail: { findings: ["rule/violation@3"] }, ...extra });

describe("the smoke cover", () => {
  it("keeps every behaviour, axis value and axis pair of a code, with fewer programs", () => {
    const rows = [
      row("a"), row("b"), row("c", { binding: "alias" }), row("d", { layer: "iife" }), row("e", { binding: "alias", layer: "iife" }),
      row("f", { outcome: "recall-gap", detail: { findings: ["rule/uncertifiable@9"] } }),
    ];
    const chosen = cover(rows);
    const all = new Set(rows.flatMap(featuresOf));
    const kept = new Set(rows.filter((r) => chosen.includes(r.id)).flatMap(featuresOf));
    expect([...all].every((feature) => kept.has(feature))).toBe(true);
    expect(chosen.length).toBeLessThan(rows.length);
    expect(cover([...rows].reverse())).toEqual(chosen);
  });

  it("ignores a finding's line in a behaviour, and holds every program judged a problem", () => {
    expect(behaviourOf(row("a"))).toBe(behaviourOf(row("b", { detail: { findings: ["rule/violation@40"] } })));
    const rows = [row("a"), row("x", { outcome: "false-clean" }), row("y", { outcome: "false-clean" })];
    expect(cover(rows).sort()).toEqual(["a", "x", "y"]);
  });
});

describe("keepIds", () => {
  const entries = ["a", "b", "c"].map((id) => ({ built: { id } }));
  const manifest = (ids, generator = GENERATOR_VERSION, seed = 1) => { const file = join(mkdtempSync(join(tmpdir(), "smoke-")), "g.json"); writeFileSync(file, JSON.stringify({ generator, seed, ids })); return file; };

  it("keeps the named programs in grid order, and refuses a manifest the grid no longer matches", () => {
    expect(keepIds(entries, manifest(["c", "a"]), "matrix", 1).map((e) => e.built.id)).toEqual(["a", "c"]);
    expect(() => keepIds(entries, manifest(["a", "z"]), "matrix", 1)).toThrow(/does not generate/);
    expect(() => keepIds(entries, manifest(["a"], GENERATOR_VERSION + 1), "matrix", 1)).toThrow(/reselect/);
    expect(() => keepIds(entries, manifest(["a"]), "matrix", 2)).toThrow(/seed 1, not 2/);
    const many = Array.from({ length: 2000 }, (_, i) => ({ built: { id: `p${i}` } }));
    const file = manifest(["p0"]);
    const pick = (sampleSeed) => keepIds(many, file, "matrix", 1, { sample: 0.05, sampleSeed }).map((e) => e.built.id);
    expect(pick(7)).toEqual(pick(7));
    expect(pick(7)).toContain("p0");
    expect(pick(7).length).toBeGreaterThan(50);
    expect(pick(7).length).toBeLessThan(160);
    expect(pick(8)).not.toEqual(pick(7));
  });
});

describe("compare", () => {
  it("counts the moved verdicts, and the kinds of move the set holds a program of", () => {
    const reference = [row("a"), row("b"), row("c", { binding: "alias" })];
    const against = [row("a"), row("b", { outcome: "false-clean", detail: {} }), row("c", { binding: "alias", outcome: "false-clean", detail: {} })];
    const held = compare(reference, against, ["b"]);
    expect(held).toMatchObject({ moved: 2, inSmoke: 1, moves: 1, movesCaught: 1, problems: 2, problemsInSmoke: 1, problemMovesMissed: [] });
    expect(compare(reference, against, ["a"])).toMatchObject({ movesCaught: 0, problemMovesMissed: [expect.stringContaining("false-clean")] });
  });
});

describe("movedVerdicts", () => {
  it("splits worse moves from the others, and names a program that gave no row", () => {
    const manifest = { ids: ["a", "b", "c", "d"], behaviours: [behaviourOf(row("x")), behaviourOf(row("x", { outcome: "recall-gap", detail: {} }))], expected: [0, 1, 0, 0] };
    const moved = movedVerdicts(manifest, [row("a", { outcome: "recall-gap", detail: {} }), row("b"), row("c")]);
    expect(moved.worse.map((m) => m.id)).toEqual(["a"]);
    expect(moved.other.map((m) => m.id)).toEqual(["b"]);
    expect(moved.missing).toEqual(["d"]);
  });
});
