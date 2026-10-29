import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { caseIdOf, chromeCodesAt, collectItems, probeOf, withBaseline, fixedStatus, itemKind, judgeWithChrome, originalId, verdictAtLine } from "./replay.mjs";

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

  it("names a violation Chrome shows no diagnostic of its codes for (a candidate false positive for a person to read)", () => {
    const built = { id: "rp-y" };
    const quiet = { row: {}, runtime: { diagnostics: [], errors: [], failure: null } };
    const store = { file: "App.tsx", rule: "no-store-mutation-outside-setter", id: "SC2003", kind: "violation", line: 8, endLine: 8 };
    const row = judgeWithChrome({ kind: "plain", chromeInput: { built, findings: [store] } }, new Map([["rp-y", quiet]]));
    expect(row.chrome).toMatchObject({ status: "violation-without-diagnostic", unmatched: ["no-store-mutation-outside-setter@8"] });
    expect(row.fixed).toBe(false);
    // A partial run never says so: the part that did not run could hold the diagnostic.
    const partial = judgeWithChrome({ kind: "plain", chromeInput: { built, findings: [store] } }, new Map([["rp-y", { row: {}, runtime: { ...quiet.runtime, failure: { phase: "halted" } } }]]));
    expect(partial.chrome.status).not.toBe("violation-without-diagnostic");
    // A rule with no runtime counterpart is never contradicted by silence.
    const noCounterpart = judgeWithChrome({ kind: "plain", chromeInput: { built, findings: [{ ...store, rule: "a-static-only-rule" }] } }, new Map([["rp-y", quiet]]));
    expect(noCounterpart.chrome.status).toBe("no-defect");
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

describe("probe witnesses", () => {
  const store = { file: "App.tsx", rule: "no-store-mutation-outside-setter", id: "SC2003", kind: "violation", line: 8, endLine: 8 };
  const witness = { selector: "#w", text: "1" };
  const built = { id: "rp-z", sites: [{ id: "w", line: 0, witness }] };
  const record = (actual, diagnostics = []) => ({ row: {}, runtime: { diagnostics, errors: [], failure: null, witnesses: [{ siteId: "w", witness, present: true, actual, reached: actual === "1" }] } });
  it("reads a probe and refuses one without a witness", () => {
    const dir = mkdtempSync(join(tmpdir(), "fuzz-probe-"));
    expect(probeOf(dir)).toBeNull();
    writeFileSync(join(dir, "probe.json"), JSON.stringify({ witness }));
    expect(probeOf(dir)).toEqual({ witness });
    writeFileSync(join(dir, "probe.json"), JSON.stringify({ witness: { selector: "#w" } }));
    expect(() => probeOf(dir)).toThrow(/witness/);
  });
  it("a violation Chrome is silent on, where the write landed, is a witnessed false positive", () => {
    const row = judgeWithChrome({ kind: "plain", chromeInput: { built, findings: [store] } }, new Map([["rp-z", record("1")]]));
    expect(row.chrome).toMatchObject({ status: "violation-without-diagnostic", witness: "landed", witnessed: "false-positive-witnessed" });
    expect(judgeWithChrome({ kind: "plain", chromeInput: { built, findings: [store] } }, new Map([["rp-z", record("0")]])).chrome.witnessed).toBe("violation-witnessed");
  });
  it("a violation the runtime is silent on whose witness did not land is backed by the behaviour; with none it is a defect", () => {
    const read = { ...store, rule: "strict-read-untracked", id: "SC1001" };
    expect(judgeWithChrome({ kind: "plain", chromeInput: { built, findings: [read] } }, new Map([["rp-z", record("0")]])).chrome).toMatchObject({ status: "violation-without-diagnostic", witnessed: "violation-witnessed" });
    expect(judgeWithChrome({ kind: "plain", chromeInput: { built, findings: [] } }, new Map([["rp-z", record("0")]])).chrome).toMatchObject({ status: "no-defect", witnessed: "defect-witnessed" });
    expect(judgeWithChrome({ kind: "plain", chromeInput: { built, findings: [] } }, new Map([["rp-z", record("1")]])).chrome.witnessed).toBeUndefined();
  });
  it("a false clean whose write did not land is a witnessed defect", () => {
    const fired = record("0", [{ code: "STORE_WRITE_OUTSIDE_SETTER", positions: [{ file: "App.tsx", line: 11 }] }]);
    expect(judgeWithChrome({ kind: "plain", chromeInput: { built, findings: [] } }, new Map([["rp-z", fired]])).chrome).toMatchObject({ status: "false-clean", witness: "not-landed", witnessed: "defect-witnessed" });
  });
  it("judges a diagnostic Chrome could not place in the file, and a code no rule claims is unclaimed", () => {
    const unplaced = { row: {}, runtime: { ...record("1").runtime, unattributed: ["UNCOMMITTED_INPUT_OVERWRITTEN", "STORE_WRITE_OUTSIDE_SETTER"] } };
    expect(judgeWithChrome({ kind: "plain", chromeInput: { built, findings: [] } }, new Map([["rp-z", unplaced]])).chrome.diagnostics).toEqual([
      { code: "UNCOMMITTED_INPUT_OVERWRITTEN", line: null, status: "unclaimed", unattributed: true },
      { code: "STORE_WRITE_OUTSIDE_SETTER", line: null, status: "false-clean", unattributed: true }]);
    const onlyUnclaimed = { row: {}, runtime: { ...record("1").runtime, unattributed: ["UNCOMMITTED_INPUT_OVERWRITTEN"] } };
    expect(judgeWithChrome({ kind: "plain", chromeInput: { built, findings: [] } }, new Map([["rp-z", onlyUnclaimed]])).chrome.status).toBe("unclaimed");
  });
});

describe("rediscovery", () => {
  it("marks an item whose judgement differs under the earlier checker, and keeps the earlier one beside it", () => {
    const now = { kind: "plain", now: "checked", findings: ["no-store-mutation-outside-setter/violation@13"] };
    const fixed = withBaseline(now, { kind: "plain", now: "checked", findings: [], chromeInput: {} });
    expect(fixed.before).toEqual({ now: "checked", findings: [], sites: undefined, changed: true });
    expect(withBaseline(now, { ...now }).before.changed).toBe(false);
    const pair = { kind: "pair", now: "high", sites: [{ site: "s1", now: "high", original: { verdict: "violation" }, variant: { verdict: "none" }, recorded: "high" }] };
    expect(withBaseline(pair, { ...pair, now: "agree", sites: [{ ...pair.sites[0], now: "agree" }] }).before.changed).toBe(true);
  });
});
