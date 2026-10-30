import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { GENERATOR_VERSION } from "./generate.mjs";
import { behaviourOf, compare, cover, featuresOf, keepIds } from "./smoke.mjs";

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
  const manifest = (ids, generator = GENERATOR_VERSION) => { const file = join(mkdtempSync(join(tmpdir(), "smoke-")), "g.json"); writeFileSync(file, JSON.stringify({ generator, ids })); return file; };

  it("keeps the named programs in grid order, and refuses a manifest the grid no longer matches", () => {
    expect(keepIds(entries, manifest(["c", "a"]), "matrix").map((e) => e.built.id)).toEqual(["a", "c"]);
    expect(() => keepIds(entries, manifest(["a", "z"]), "matrix")).toThrow(/does not generate/);
    expect(() => keepIds(entries, manifest(["a"], GENERATOR_VERSION + 1), "matrix")).toThrow(/reselect/);
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
