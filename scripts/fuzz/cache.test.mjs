import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { BaselineCache, MismatchHistory, codeHash, environmentKey, fileHash, programKey } from "./cache.mjs";

const scratch = () => mkdtempSync(join(tmpdir(), "fuzz-cache-"));
const config = (dir, text = "checker") => {
  const bin = join(dir, "bin"); const typefacts = join(dir, "tf");
  writeFileSync(bin, text); writeFileSync(typefacts, "tf");
  mkdirSync(join(dir, "fast"), { recursive: true }); mkdirSync(join(dir, "oracle"), { recursive: true });
  writeFileSync(join(dir, "fast/package-lock.json"), "{}"); writeFileSync(join(dir, "oracle/package-lock.json"), "{}");
  return { bin, typefacts, fastInstall: join(dir, "fast"), install: join(dir, "oracle") };
};

describe("the environment key", () => {
  it("is stable, and every ingredient changes it", () => {
    const dir = scratch();
    const base = environmentKey(config(dir), { code: "c1" });
    expect(environmentKey(config(dir), { code: "c1" })).toBe(base);
    expect(environmentKey(config(dir), { code: "c2" })).not.toBe(base);                      // the judging code
    expect(environmentKey(config(dir, "rebuilt checker"), { code: "c1" })).not.toBe(base);   // the checker binary
    const cfg = config(dir);
    writeFileSync(cfg.typefacts, "other tf");
    expect(environmentKey(cfg, { code: "c1" })).not.toBe(environmentKey(config(scratch()), { code: "c1" }));   // the type-facts producer
    const locked = config(scratch());
    const before = environmentKey(locked, { code: "c1" });
    writeFileSync(join(locked.fastInstall, "package-lock.json"), '{"changed":1}');
    expect(environmentKey(locked, { code: "c1" })).not.toBe(before);                         // the fast install's lockfile
  });
  it("hashes the judging sources, not the tests", () => {
    const dir = scratch();
    writeFileSync(join(dir, "classify.mjs"), "a"); writeFileSync(join(dir, "classify.test.mjs"), "t1");
    const one = codeHash(dir);
    writeFileSync(join(dir, "classify.test.mjs"), "t2");
    expect(codeHash(dir)).toBe(one);
    writeFileSync(join(dir, "classify.mjs"), "b");
    expect(codeHash(dir)).not.toBe(one);
  });
  it("a missing file hashes as missing, not as an error", () => {
    expect(fileHash("/nonexistent/file")).toBe("missing");
  });
});

describe("BaselineCache", () => {
  const program = { seed: 7, profile: "body-call" };
  it("returns what was stored under the same key, counts hits and misses, and ignores another key", () => {
    const dir = scratch();
    const first = new BaselineCache(dir, "k".repeat(64));
    expect(first.get(program)).toBeNull();
    first.set(program, { id: "fuzz-7", verdict: "agree" });
    first.save();
    const again = new BaselineCache(dir, "k".repeat(64));
    expect(again.get(program)).toEqual({ id: "fuzz-7", verdict: "agree" });
    expect(again.get({ ...program, seed: 8 })).toBeNull();
    expect(again.get({ ...program, profile: "async" })).toBeNull();
    expect([again.hits, again.misses]).toEqual([1, 2]);
    expect(new BaselineCache(dir, "j".repeat(64)).get(program)).toBeNull();
  });
  it("keys programs by profile, component count and seed", () => {
    expect(programKey({ seed: 1, profile: undefined })).toBe("-|1|1");
    expect(programKey({ seed: 1, profile: "x", components: 3 })).toBe("x|3|1");
  });
});

describe("MismatchHistory", () => {
  it("puts what mismatched last time first, keeps the rest in order, and forgets what now matches", () => {
    const dir = scratch();
    const history = new MismatchHistory(dir);
    history.record("profiles", "async", true);
    history.record("programs", "async|1|9", true);
    history.save();
    const next = new MismatchHistory(dir);
    expect(next.order(["a", "b", "async", "c"], (x) => x, "profiles")).toEqual(["async", "a", "b", "c"]);
    expect(next.order([{ s: 3 }, { s: 9 }, { s: 5 }], (p) => `async|1|${p.s}`, "programs")).toEqual([{ s: 9 }, { s: 3 }, { s: 5 }]);
    next.record("profiles", "async", false);
    expect(next.order(["a", "async"], (x) => x, "profiles")).toEqual(["a", "async"]);
  });
});
