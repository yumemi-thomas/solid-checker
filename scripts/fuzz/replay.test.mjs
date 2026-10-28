import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { caseIdOf, chromeCodesAt, collectItems, fixedStatus, itemKind, judgeWithChrome, originalId, verdictAtLine } from "./replay.mjs";

const scratch = () => mkdtempSync(join(tmpdir(), "fuzz-replay-"));
const write = (dir, files) => { mkdirSync(dir, { recursive: true }); for (const [name, text] of Object.entries(files)) writeFileSync(join(dir, name), text); return dir; };

describe("the items", () => {
  it("knows a pair, a text pair and a plain repro by their files, and searches other directories", () => {
    const root = scratch();
    const pair = write(join(root, "run/divergences/fuzz-1-eq0-accessor-form"), { "original.program.json": "{}", "variant.program.json": "{}", "divergence.json": "[]", "original.App.tsx": "a", "variant.App.tsx": "b" });
    const text = write(join(root, "repros/abc"), { "original.App.tsx": "a", "variant.App.tsx": "b", "divergence.json": "{}" });
    const plain = write(join(root, "min/rc4"), { "App.tsx": "a" });
    write(join(root, "run/cases/fuzz-1"), { "App.tsx": "not an item: a run's own case directory" });
    expect([itemKind(pair), itemKind(text), itemKind(plain), itemKind(root)]).toEqual(["pair", "text-pair", "plain", null]);
    expect(collectItems([root]).map((item) => [item.kind, item.dir])).toEqual([["plain", plain], ["text-pair", text], ["pair", pair]]);
  });

  it("reads a list file with labels and comments, and names each directory once", () => {
    const root = scratch();
    const plain = write(join(root, "min/rc4"), { "App.tsx": "a" });
    const list = join(root, "list.txt");
    writeFileSync(list, `# a comment\n${plain} A1 first\n\n${plain} again\n`);
    expect(collectItems([list])).toEqual([{ dir: plain, kind: "plain", label: "A1 first" }]);
    expect(() => collectItems([join(root, "missing")])).toThrow(/no such/);
  });

  it("takes the original's id from a variant id", () => {
    expect(originalId("fuzz-2436666819-eq0-accessor-form")).toBe("fuzz-2436666819");
    expect(originalId("fuzz-535203443-eq2-outline")).toBe("fuzz-535203443");
  });
});

describe("judging", () => {
  const violation = { file: "App.tsx", rule: "action-called-in-owned-scope", id: "SC", kind: "violation", line: 4, endLine: 4 };
  it("reads the verdict at a recorded line as siteVerdicts does", () => {
    expect(verdictAtLine([violation], 4)).toMatchObject({ verdict: "violation", rules: ["action-called-in-owned-scope"] });
    expect(verdictAtLine([violation], 5)).toMatchObject({ verdict: "none", rules: [] });
  });

  it("calls an item fixed only when the sides agree on a violation whose rule claims what Chrome reports", () => {
    expect(fixedStatus({ now: "agree", verdict: "violation", rules: ["action-called-in-owned-scope"], chromeCodes: ["ACTION_CALLED_IN_OWNED_SCOPE"] })).toBe(true);
    expect(fixedStatus({ now: "agree", verdict: "violation", rules: ["action-called-in-owned-scope"], chromeCodes: [] })).toBe(false);        // Chrome no longer shows it
    expect(fixedStatus({ now: "agree", verdict: "none", rules: [], chromeCodes: ["ACTION_CALLED_IN_OWNED_SCOPE"] })).toBe(false);              // both sides clean: worse, not fixed
    expect(fixedStatus({ now: "agree", verdict: "uncertifiable", rules: ["action-called-in-owned-scope"], chromeCodes: ["ACTION_CALLED_IN_OWNED_SCOPE"] })).toBe(false);
    expect(fixedStatus({ now: "high", verdict: "violation", rules: ["action-called-in-owned-scope"], chromeCodes: ["ACTION_CALLED_IN_OWNED_SCOPE"] })).toBe(false);
  });

  it("reads Chrome's defect codes at a line and a partial run", () => {
    const record = { runtime: { diagnostics: [{ code: "ACTION_CALLED_IN_OWNED_SCOPE", positions: [{ file: "App.tsx", line: 4 }] }, { code: "REACTIVITY_HALTED", positions: [{ file: "App.tsx", line: 4 }] }], errors: [], failure: null } };
    expect(chromeCodesAt(record, 4)).toEqual({ codes: ["ACTION_CALLED_IN_OWNED_SCOPE"], partial: false });
    expect(chromeCodesAt(record, 9)).toEqual({ codes: [], partial: false });
    expect(chromeCodesAt(record, 9, [{ ...violation, line: 3, endLine: 5 }]).codes).toEqual(["ACTION_CALLED_IN_OWNED_SCOPE"]);
    expect(chromeCodesAt({ runtime: { ...record.runtime, failure: { phase: "halted" } } }, 4).partial).toBe(true);
  });

  it("names a cached Chrome run on the row, and judges a plain repro by the diagnostics Chrome reports", () => {
    const built = { id: "rp-x" };
    const runtime = { diagnostics: [{ code: "NO_OWNER_CLEANUP", positions: [{ file: "App.tsx", line: 4 }] }], errors: [], failure: null };
    const covered = { kind: "plain", chromeInput: { built, findings: [{ ...violation, rule: "missing-owner" }] } };
    const hit = judgeWithChrome(covered, new Map([["rp-x", { row: {}, runtime, cached: true }]]));
    expect(hit).toMatchObject({ chrome: { status: "agree" }, chromeCache: "hit", fixed: true });
    const missed = judgeWithChrome({ kind: "plain", chromeInput: { built, findings: [] } }, new Map([["rp-x", { row: {}, runtime, cached: false }]]));
    expect(missed).toMatchObject({ chrome: { status: "false-clean", diagnostics: [{ code: "NO_OWNER_CLEANUP", line: 4, status: "false-clean" }] }, chromeCache: "miss", fixed: false });
    const uncached = judgeWithChrome({ kind: "plain", chromeInput: { built, findings: [] } }, new Map([["rp-x", { row: {}, runtime }]]));
    expect(uncached.chromeCache).toBeUndefined();
  });

  it("never reads Chrome's silence on a bare text pair as evidence: it has no driver plan", () => {
    const site = { site: "s1", line: 4, variantLine: 4, now: "high", original: { verdict: "violation", rules: ["action-called-in-owned-scope"] } };
    const row = { kind: "text-pair", chromeInput: { originalBuilt: { id: "o" }, variantBuilt: { id: "v" }, sites: [site] } };
    const quiet = { row: {}, runtime: { diagnostics: [], errors: [], failure: null } };
    expect(judgeWithChrome(row, new Map([["o", quiet], ["v", quiet]])).chrome).toMatchObject({ status: "inconclusive", sites: [{ why: "no-driver-plan" }] });
    const loud = { row: {}, runtime: { diagnostics: [{ code: "ACTION_CALLED_IN_OWNED_SCOPE", positions: [{ file: "App.tsx", line: 4 }] }], errors: [], failure: null } };
    expect(judgeWithChrome(row, new Map([["o", loud], ["v", quiet]])).chrome.status).toBe("defect-at-site");
  });
});

describe("case ids", () => {
  it("are stable for a directory and differ for two directories of one name", () => {
    expect(caseIdOf({ dir: "/a/min/rc4" })).toBe(caseIdOf({ dir: "/a/min/rc4" }));
    expect(caseIdOf({ dir: "/a/min/rc4" })).not.toBe(caseIdOf({ dir: "/b/min/rc4" }));
    expect(caseIdOf({ dir: "/a/min/rc4.x y" })).toMatch(/^rp-rc4_x_y-[0-9a-f]{6}$/);
  });
});
