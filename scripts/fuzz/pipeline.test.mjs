import { existsSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { evaluate, signaturesOf } from "./pipeline.mjs";

// The wiring with fakes for the three expensive steps: the type checker, the checker, the Happy DOM batch.
const entry = (seed) => { const program = generateProgram(seed, { profile: "in-place" }); return { program, built: buildCase(program, { id: `fuzz-${seed}` }) }; };
const config = () => ({ install: mkdtempSync(join(tmpdir(), "fuzz-pipe-")), casesDir: mkdtempSync(join(tmpdir(), "fuzz-cases-")), concurrency: 2, workers: 1 });
const okTypes = async () => ({ ok: true, errors: [] });

describe("evaluate", () => {
  it("drops a program that does not type-check, before the checker and the runtime see it", async () => {
    const e = entry(1);
    let ran = false;
    const results = await evaluate([e], config(), {
      typecheck: async () => ({ ok: false, errors: ["App.tsx(1,1): error TS2322"] }),
      runChecker: async () => { ran = true; return { ok: true, findings: [] }; },
      runFast: async () => { ran = true; return new Map(); },
    });
    expect(ran).toBe(false);
    expect(results.get(e.built.id).classification).toMatchObject({ verdict: "type-error" });
    expect(existsSync(join(config().casesDir))).toBe(true);
  });

  it("passes only the cases the checker answered to the runtime batch, and classifies a checker failure as inconclusive", async () => {
    const [a, b] = [entry(2), entry(3)];
    let batch = [];
    const results = await evaluate([a, b], config(), {
      typecheck: okTypes,
      runChecker: async (dir) => (dir.endsWith(a.built.id) ? { ok: true, findings: [] } : { ok: false, findings: [], error: "timeout" }),
      runFast: async (cases) => { batch = cases.map((c) => c.id); return new Map(cases.map((c) => [c.id, { diagnostics: [], errors: [], witnesses: [] }])); },
    });
    expect(batch).toEqual([a.built.id]);
    expect(results.get(a.built.id).classification.verdict).toBe("agree");
    expect(results.get(b.built.id).classification).toMatchObject({ verdict: "inconclusive", reasons: [expect.stringMatching(/checker failed: timeout/)] });
  });

  it("finds a candidate false positive end to end, and gives it a signature", async () => {
    const e = entry(4);
    const [site] = e.built.sites;
    const results = await evaluate([e], config(), {
      typecheck: okTypes,
      runChecker: async () => ({ ok: true, findings: [{ file: "App.tsx", rule: site.kind === "write" ? "no-write-in-pure-computation" : "strict-read-untracked", id: "SC", kind: "violation", line: site.line, endLine: site.line }] }),
      runFast: async (cases) => new Map(cases.map((c) => [c.id, { diagnostics: [], errors: [], witnesses: e.built.sites.map((s) => ({ siteId: s.id, reached: true })) }])),
    });
    const result = results.get(e.built.id);
    expect(result.classification.verdict).toBe("candidate-false-positive");
    expect(result.verdicts[0]).toMatchObject({ siteId: site.id, verdict: "violation" });
    const [found] = signaturesOf(e.program, e.built, result.classification);
    expect(found.signature).toMatch(/^(no-write-in-pure-computation|strict-read-untracked)\|candidate-false-positive\|silent\|component\.body/);
  });
});
