// Run: bun packages/cli/node_modules/vitest/vitest.mjs run --config packages/cli/vitest.config.mjs scripts/fuzz/witness.test.mjs
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { probeOf } from "./replay.mjs";
import { SPEC_RULES, TEMPLATES, WITNESS_RULES, judgeWitness, writeProbes } from "./witness.mjs";

const f = (rule, kind, line = 5) => ({ rule, kind, line, endLine: line, file: "App.tsx" });

describe("the witness oracle", () => {
  it("has a defect form and a live twin for every rule, each a probe replay.mjs reads", () => {
    for (const rule of WITNESS_RULES) {
      expect(TEMPLATES.some((t) => t.rule === rule && t.expect === "stale"), rule).toBe(true);
      expect(TEMPLATES.some((t) => t.rule === rule && t.expect === "live"), rule).toBe(true);
    }
    for (const rule of SPEC_RULES) expect(new Set(TEMPLATES.filter((t) => t.rule === rule).map((t) => t.expect))).toEqual(new Set(["flag", "silent"]));
    const dirs = writeProbes(mkdtempSync(join(tmpdir(), "fuzz-witness-")));
    expect(dirs.length).toBe(TEMPLATES.length);
    for (const { dir } of dirs) expect(probeOf(dir).witness.selector).toBe("#w");
    expect(readFileSync(join(dirs[0].dir, "App.tsx"), "utf8")).toContain('id="w"');
  });
  it("judges by what the page shows: stale UI backs a violation, live UI contradicts one", () => {
    const rule = "no-destructure";
    expect(judgeWitness({ rule, expect: "stale", findings: [f(rule, "violation")], witness: "not-landed" }).outcome).toBe("agree");
    expect(judgeWitness({ rule, expect: "stale", findings: [f(rule, "uncertifiable")], witness: "not-landed" }).outcome).toBe("recall-gap");
    expect(judgeWitness({ rule, expect: "stale", findings: [], witness: "not-landed" }).outcome).toBe("false-clean");
    expect(judgeWitness({ rule, expect: "live", findings: [f(rule, "violation")], witness: "landed" }).outcome).toBe("false-positive");
    expect(judgeWitness({ rule, expect: "live", findings: [], witness: "landed" }).outcome).toBe("clean");
    expect(judgeWitness({ rule, expect: "stale", findings: [], witness: "absent" }).outcome).toBe("inconclusive");
    // A template Chrome shows the other way is a template to fix, said so; another rule's finding is not this rule's answer.
    expect(judgeWitness({ rule, expect: "stale", findings: [f("strict-read-untracked", "violation")], witness: "landed" })).toMatchObject({ outcome: "clean", detail: { unexpected: "designed stale, Chrome shows live" } });
  });
  it("without Chrome judges by the design, as candidates; the style rules by the specification; a typed-away program by tsc-overlap", () => {
    expect(judgeWitness({ rule: "uncalled-accessor", expect: "stale", findings: [] }).outcome).toBe("static-miss");
    expect(judgeWitness({ rule: "uncalled-accessor", expect: "live", findings: [f("uncalled-accessor", "violation")] }).outcome).toBe("static-flagged");
    expect(judgeWitness({ rule: "prefer-for", expect: "flag", findings: [] }).outcome).toBe("spec-miss");
    expect(judgeWitness({ rule: "prefer-for", expect: "silent", findings: [f("prefer-for", "violation")] }).outcome).toBe("spec-flagged");
    const typed = { ok: false, errors: ["App.tsx(5,3): error TS2362: no."] };
    expect(judgeWitness({ rule: "uncalled-accessor", expect: "stale", findings: [f("uncalled-accessor", "violation")], typed }).outcome).toBe("tsc-overlap");
  });
});
