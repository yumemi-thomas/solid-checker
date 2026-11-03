// Run: bun packages/cli/node_modules/vitest/vitest.mjs run --config packages/cli/vitest.config.mjs scripts/fuzz/tsc-overlap.test.mjs
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { DEFAULT_ORACLE_INSTALL } from "./chrome.mjs";
import { materializeForChecker } from "./checker.mjs";
import { generateLegal, judgeLegal, legalCells } from "./legal.mjs";
import { buildMatrix, cellList, gate, generateShape, judge, regionOf, siteOf } from "./matrix.mjs";
import { SPEC, TYPED_VARIANTS, recallVariantsOf, variantsOf } from "./misuse.mjs";
import { resolveTsc, typecheck } from "./typecheck.mjs";
import { evaluate } from "./pipeline.mjs";
import { checkTypedAway, tscOverlap, typeErrorsIn, typeRejectedRows, typedAwayVerdict, typedOnPurpose } from "./tsc-overlap.mjs";

const finding = (rule, kind, line, endLine = line) => ({ rule, kind, line, endLine, id: "SC", file: "App.tsx" });
const rejected = (...lines) => ({ ok: false, errors: lines.map((line) => `App.tsx(${line},7): error TS2345: Argument of type 'x' is not assignable.`) });

describe("the tsc overlap of a typed-away program", () => {
  it("reads the App.tsx errors of a failed type check, by line", () => {
    const typed = { ok: false, errors: ["App.tsx(12,5): error TS2345: no.", "helpers.ts(3,1): error TS2322: other.", "garbage"] };
    expect(typeErrorsIn(typed)).toEqual([{ line: 12, column: 5, code: "TS2345", message: "no." }]);
    expect(typeErrorsIn(typed, "helpers.ts")).toMatchObject([{ line: 3, code: "TS2322" }]);
  });
  it("is an overlap when a violation's lines hold a rejected line, or a claiming violation answers a rejected site", () => {
    expect(tscOverlap({ typed: rejected(10), checker: { ok: true, findings: [finding("missing-owner", "violation", 9, 11)] } }).overlaps).toEqual(["missing-owner@9~TS2345@10"]);
    // A claiming rule's violation anchored at the call that runs the misuse (remote), the site itself rejected.
    expect(tscOverlap({ typed: rejected(10), checker: { ok: true, findings: [finding("missing-owner", "violation", 20)] }, region: [10], rules: ["missing-owner"] }).overlaps).toEqual(["missing-owner@20~TS2345@10"]);
    // Another rule's remote violation, an uncertifiable finding on the line, a violation elsewhere: no overlap.
    const found = tscOverlap({ typed: rejected(10), checker: { ok: true, findings: [finding("strict-read-untracked", "violation", 20), finding("missing-owner", "uncertifiable", 10)] }, region: [10], rules: ["missing-owner"] });
    expect(found).toMatchObject({ overlaps: [], elsewhere: ["strict-read-untracked@20"], errors: ["TS2345@10"] });
  });
  it("keeps a typed-away program typed-away without a checker run, with a failed one, or with no overlapping violation", () => {
    expect(typedAwayVerdict({ typed: rejected(10) }).outcome).toBe("typed-away");
    expect(typedAwayVerdict({ typed: rejected(10), checker: { ok: false, error: "boom", findings: [] } })).toMatchObject({ outcome: "typed-away", detail: { checker: "failed: boom" } });
    expect(typedAwayVerdict({ typed: rejected(10), checker: { ok: true, findings: [finding("missing-owner", "violation", 30)] } })).toMatchObject({ outcome: "typed-away", detail: { elsewhere: ["missing-owner@30"] } });
  });
});

describe("the grids judge a typed-away program", () => {
  const site = { line: 10, aux: [], hit: 4 };
  const overlapping = { ok: true, findings: [finding("no-store-mutation-outside-setter", "violation", 10)] };
  it("misuse and legal alike: tsc-overlap on a violation over the rejected code, typed-away otherwise", () => {
    expect(judge({ code: "STORE_WRITE_OUTSIDE_SETTER", site, typecheck: rejected(10), checker: overlapping }).outcome).toBe("tsc-overlap");
    expect(judgeLegal({ code: "STORE_WRITE_OUTSIDE_SETTER", site, typecheck: rejected(10), checker: overlapping }).outcome).toBe("tsc-overlap");
    expect(judge({ code: "STORE_WRITE_OUTSIDE_SETTER", site, typecheck: rejected(10), checker: { ok: true, findings: [] } }).outcome).toBe("typed-away");
    expect(judgeLegal({ code: "STORE_WRITE_OUTSIDE_SETTER", site, typecheck: rejected(10), checker: undefined }).outcome).toBe("typed-away");
  });
  it("the gate fails a program that enters tsc-overlap, from a typed-away baseline too", () => {
    const row = (seed, outcome) => ({ code: "A", context: "body", seed, outcome, binding: "direct", layer: "none" });
    const matrixOf = (rows) => buildMatrix(rows, { options: { seed: 1, perCell: 3 }, header: {}, typedAway: {} });
    const result = gate(matrixOf([row(1, "agree"), row(2, "typed-away"), row(3, "agree")]), matrixOf([row(1, "agree"), row(2, "tsc-overlap"), row(3, "agree")]));
    expect(result.ok).toBe(false);
    expect(result.regressions).toEqual([{ cell: "A|body", why: "tsc-overlap 0 -> 1", programs: ["2: typed-away -> tsc-overlap"] }]);
  });
});

describe("the checker run on a typed-away program (checkTypedAway)", () => {
  const entry = () => { const program = generateProgram(7, { profile: "in-place" }); return { program, built: buildCase(program, { id: "fuzz-7" }) }; };
  const config = () => ({ install: mkdtempSync(join(tmpdir(), "fuzz-tsc-")), casesDir: mkdtempSync(join(tmpdir(), "fuzz-tsc-cases-")), concurrency: 1, workers: 1 });
  it("evaluate leaves it unchecked and unrun; checkTypedAway runs the checker on it alone", async () => {
    const log = [];
    const deps = { typecheck: async () => rejected(1), runChecker: async () => { log.push("checker"); return { ok: true, findings: [] }; }, runFast: async () => { log.push("fast"); return new Map(); } };
    const cfg = config();
    const results = await evaluate([entry()], cfg, deps);
    expect(log).toEqual([]);
    expect(results.get("fuzz-7").checker).toBeUndefined();
    expect(await checkTypedAway(results, cfg, { runChecker: deps.runChecker })).toBe(1);
    expect(log).toEqual(["checker"]);
    expect(results.get("fuzz-7")).toMatchObject({ typecheck: { ok: false }, checker: { ok: true } });
    // Once checked, a second call has nothing to do; a checker that throws is the program's harness answer.
    expect(await checkTypedAway(results, cfg, { runChecker: deps.runChecker })).toBe(0);
    const again = await evaluate([entry()], config(), deps);
    await checkTypedAway(again, cfg, { runChecker: async () => { throw new Error("boom"); } });
    expect(again.get("fuzz-7").checker).toMatchObject({ ok: false, error: "boom" });
  });
});

// The variants typed away on purpose are rejected by tsrs against the published typings, and the SC7001 cast forms are not (they are
// the rule's runtime-backed cells); skipped where tsrs or the install is not there.
const TSC = resolveTsc();
const typedHere = existsSync(TSC) && existsSync(join(DEFAULT_ORACLE_INSTALL, "node_modules/solid-js/package.json"));
describe.skipIf(!typedHere)("the typed-away templates (misuse.mjs TYPED_VARIANTS)", () => {
  it("every one is rejected with a TS error on the site's line, and the cast forms type-check", async () => {
    const programs = [];
    for (const [code, variants] of Object.entries(TYPED_VARIANTS)) for (const variant of variants) {
      const legal = !recallVariantsOf(code).includes(variant);
      const cell = (legal ? legalCells({ codes: [code], recall: true }) : cellList({ codes: [code], recall: true })).find(({ context }) => !SPEC[code].variantContexts?.[variant] || SPEC[code].variantContexts[variant].includes(context));
      programs.push({ want: "rejected", code, variant, ...(legal ? generateLegal({ seed: 1 }, cell, { shape: { variant, binding: "direct", layer: "none" } }) : generateShape({ seed: 1 }, cell, { variant, binding: "direct", layer: "none" }, 0)) });
    }
    for (const variant of variantsOf("MISSING_EFFECT_FN")) programs.push({ want: "ok", code: "MISSING_EFFECT_FN", variant, ...generateShape({ seed: 1 }, { code: "MISSING_EFFECT_FN", context: "body" }, { variant, binding: "direct", layer: "none" }, 0) });
    const failures = [];
    for (const { want, code, variant, built } of programs) {
      const dir = materializeForChecker(built, DEFAULT_ORACLE_INSTALL, `tsc-overlap-${built.id}`);
      try {
        const answer = await typecheck(dir, { tsc: TSC });
        const site = siteOf(built);
        const onSite = typeErrorsIn(answer).some((error) => regionOf(site).includes(error.line));
        if (want === "ok" ? !answer.ok : answer.ok || !onSite) failures.push(`${code}/${variant}: ${answer.ok ? "type-checks" : answer.errors[0]}`);
      } finally { rmSync(dir, { recursive: true, force: true }); }
    }
    expect(failures).toEqual([]);
  }, 120000);
});

describe("the type-error refusal and the variants typed away on purpose", () => {
  it("does not count a typed-on-purpose row with a real TS error, and counts every other rejection", () => {
    const row = (outcome, variant, errors) => ({ code: "STRICT_READ_UNTRACKED", variant, outcome, detail: { errors } });
    expect(typedOnPurpose(row("typed-away", "uncalled-arith", ["TS2362@5"]))).toBe(true);
    expect(typedOnPurpose(row("tsc-overlap", "uncalled-arith", ["TS2362@5"]))).toBe(true);
    // A broken type checker prints no TS error on App.tsx; a base variant typed away is the generator's problem; a clean row is not rejected.
    expect(typedOnPurpose(row("typed-away", "uncalled-arith", []))).toBe(false);
    expect(typedOnPurpose(row("typed-away", "signal", ["TS2362@5"]))).toBe(false);
    expect(typedOnPurpose(row("agree", "uncalled-arith", undefined))).toBe(false);
    expect(typeRejectedRows([row("typed-away", "uncalled-arith", ["TS2362@5"]), row("typed-away", "signal", ["TS2322@4"]), row("agree", "signal")]).length).toBe(1);
  });
});
