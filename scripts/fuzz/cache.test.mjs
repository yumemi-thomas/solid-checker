import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { BaselineCache, EntryCache, MismatchHistory, ORACLE_SOURCES, cacheableRecord, cacheableTypecheck, caseFiles, caseKey, chromeEnvironmentKey, codeHash, environmentKey, fileHash, programKey, runtimeEnvironmentKey, treeHash } from "./cache.mjs";

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

describe("the runtime and Chrome keys", () => {
  const setup = () => {
    const dir = scratch();
    const tsc = join(dir, "tsrs"); writeFileSync(tsc, "tsrs");
    mkdirSync(join(dir, "fast"), { recursive: true }); mkdirSync(join(dir, "oracle"), { recursive: true });
    writeFileSync(join(dir, "fast/package-lock.json"), "{}"); writeFileSync(join(dir, "oracle/package-lock.json"), "{}");
    return { dir, config: { tsc, fastInstall: join(dir, "fast"), install: join(dir, "oracle"), virtualClock: true, bin: join(dir, "checker") } };
  };

  it("the runtime key moves with the judging sources, the oracle, the clock, the type checker and either lockfile, and not with the checker", () => {
    const { dir, config } = setup();
    const key = (over = {}, parts = {}) => runtimeEnvironmentKey({ ...config, ...over }, { code: "c1", oracle: "o1", ...parts });
    const base = key();
    expect(key()).toBe(base);
    expect(key({}, { code: "c2" })).not.toBe(base);                     // a judging source
    expect(key({}, { oracle: "o2" })).not.toBe(base);                   // the oracle's entry (the type check compiles it)
    expect(key({ virtualClock: false })).not.toBe(base);                // the clock
    writeFileSync(config.bin, "a rebuilt checker");
    expect(key()).toBe(base);                                           // the checker is not in it: that is the point
    writeFileSync(config.tsc, "another tsrs");
    const tsc = key();
    expect(tsc).not.toBe(base);                                         // the type checker binary
    writeFileSync(join(config.fastInstall, "package-lock.json"), '{"a":1}');
    const fast = key();
    expect(fast).not.toBe(tsc);                                         // the fast install's lockfile
    writeFileSync(join(config.install, "package-lock.json"), '{"b":1}');
    expect(key()).not.toBe(fast);                                       // the oracle install's lockfile
    expect(dir).toBeTruthy();
  });

  it("the Chrome key moves with the judging sources, the oracle and its product scripts, the browser and the oracle lockfile, and not with the checker", () => {
    const { config } = setup();
    const key = (over = {}, parts = {}) => chromeEnvironmentKey({ ...config, ...over }, { code: "c1", oracle: "o1", browser: "chromium-1", ...parts });
    const base = key();
    expect(key({}, { code: "c2" })).not.toBe(base);
    expect(key({}, { oracle: "o2" })).not.toBe(base);
    expect(key({}, { browser: "chromium-2" })).not.toBe(base);
    writeFileSync(config.bin, "a rebuilt checker");
    expect(key()).toBe(base);
    writeFileSync(join(config.install, "package-lock.json"), '{"b":1}');
    expect(key()).not.toBe(base);
    expect(chromeEnvironmentKey(config, { code: "c1", oracle: "o1", browser: "chromium-1" })).not.toBe(runtimeEnvironmentKey(config, { code: "c1", oracle: "o1" }));
  });

  it("the oracle sources hash covers runtime-oracle.mjs and the product's feedback scripts", () => {
    expect(ORACLE_SOURCES()).toMatch(/^[0-9a-f]{64}$/);
    const dir = scratch();
    writeFileSync(join(dir, "a.mjs"), "1"); writeFileSync(join(dir, "a.test.mjs"), "t");
    const one = treeHash(dir, (name) => !/\.test\./.test(name));
    writeFileSync(join(dir, "a.test.mjs"), "t2");
    expect(treeHash(dir, (name) => !/\.test\./.test(name))).toBe(one);
    writeFileSync(join(dir, "a.mjs"), "2");
    expect(treeHash(dir, (name) => !/\.test\./.test(name))).not.toBe(one);
  });

  it("a case key moves with any file of the case and with the part asked, never with the order of the files", () => {
    const files = { "App.tsx": "a", "App.run.txt": "b", "sites.json": "{}" };
    const base = caseKey(files, "fast-run|fuzz-1");
    expect(caseKey({ "sites.json": "{}", "App.run.txt": "b", "App.tsx": "a" }, "fast-run|fuzz-1")).toBe(base);
    expect(caseKey({ ...files, "App.tsx": "a " }, "fast-run|fuzz-1")).not.toBe(base);             // the program text
    expect(caseKey({ ...files, "App.run.txt": "c" }, "fast-run|fuzz-1")).not.toBe(base);           // the twin
    expect(caseKey({ ...files, "node_modules/fz-lib/index.js": "x" }, "fast-run|fuzz-1")).not.toBe(base);   // a package file
    expect(caseKey(files, "fast-clean|fuzz-1")).not.toBe(base);                                     // the other variant
    expect(caseKey(files, "typecheck")).not.toBe(base);
    const dir = scratch();
    mkdirSync(join(dir, "node_modules/p"), { recursive: true });
    writeFileSync(join(dir, "App.tsx"), "a"); writeFileSync(join(dir, "node_modules/p/index.js"), "x");
    expect(caseFiles(dir)).toEqual({ "App.tsx": "a", "node_modules/p/index.js": "x" });
  });
});

describe("EntryCache", () => {
  it("returns what was stored under the same environment, counts hits and misses, and keeps environments apart", () => {
    const dir = scratch();
    const one = new EntryCache(dir, "runtime", "e".repeat(64));
    expect(one.get("k1")).toBeUndefined();
    one.set("k1", { verdict: "x" });
    const again = new EntryCache(dir, "runtime", "e".repeat(64));
    expect(again.get("k1")).toEqual({ verdict: "x" });
    expect(again.get("k2")).toBeUndefined();
    expect(again.stats()).toMatchObject({ hits: 1, misses: 1, stored: 0 });
    expect(new EntryCache(dir, "runtime", "f".repeat(64)).get("k1")).toBeUndefined();
    expect(new EntryCache(dir, "chrome", "e".repeat(64)).get("k1")).toBeUndefined();
  });

  it("an entry stored under another environment in the same file is a miss", () => {
    const dir = scratch();
    const a = new EntryCache(dir, "runtime", `${"e".repeat(24)}a`);
    a.set("k1", 1);
    expect(new EntryCache(dir, "runtime", `${"e".repeat(24)}b`).get("k1")).toBeUndefined();
  });

  it("stores only what says something about the program", () => {
    expect(cacheableTypecheck({ ok: true, errors: [] })).toBe(true);
    expect(cacheableTypecheck({ ok: false, errors: ["App.tsx(1,1): error TS2322"] })).toBe(true);
    expect(cacheableTypecheck({ ok: false, errors: [] })).toBe(false);                               // killed by the timeout
    expect(cacheableTypecheck({ ok: false, errors: ["not found"], missing: true })).toBe(false);
    expect(cacheableRecord({ diagnostics: [] })).toBe(true);
    expect(cacheableRecord({ failure: { phase: "render" } })).toBe(true);
    expect(cacheableRecord({ failure: { phase: "vitest", message: "no record written" } })).toBe(false);
  });
});
