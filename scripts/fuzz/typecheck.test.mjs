import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, describe, expect, it } from "vitest";
import { DEFAULT_TSC, TYPE_ERROR_REFUSAL_SHARE, requireTypeChecker, resolveTsc, typeErrorRefusal } from "./typecheck.mjs";
import { configFrom, parseArgs } from "./run.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const scratch = mkdtempSync(join(tmpdir(), "fuzz-typecheck-"));
afterAll(() => rmSync(scratch, { recursive: true, force: true }));

describe("resolveTsc", () => {
  const exists = join(scratch, "tsrs");
  writeFileSync(exists, "#!/bin/sh\n");
  it("takes --tsc, then SOLID_CHECKER_TSRS_BIN, then FUZZ_TSC", () => {
    expect(resolveTsc("/a/tsrs", { SOLID_CHECKER_TSRS_BIN: "/b/tsrs", FUZZ_TSC: "/c/tsrs" })).toBe("/a/tsrs");
    expect(resolveTsc(undefined, { SOLID_CHECKER_TSRS_BIN: "/b/tsrs", FUZZ_TSC: "/c/tsrs" })).toBe("/b/tsrs");
    expect(resolveTsc(undefined, { FUZZ_TSC: "/c/tsrs" })).toBe("/c/tsrs");
  });
  it("falls back to a tsrs beside a checkout, and otherwise names the default path so the refusal can show it", () => {
    const resolved = resolveTsc(undefined, {});
    expect(resolved === DEFAULT_TSC || existsSync(resolved)).toBe(true);
  });
  it("is what a run's configuration carries", () => {
    expect(configFrom(parseArgs(["--tsc", "/x/tsrs"]), "out").tsc).toBe("/x/tsrs");
  });
});

describe("requireTypeChecker", () => {
  it("refuses a path that does not exist", () => {
    expect(() => requireTypeChecker(join(scratch, "missing"))).toThrow(/type checker not found/);
    expect(() => requireTypeChecker(undefined)).toThrow(/type checker not found/);
  });
  it("accepts one that does", () => {
    const real = join(scratch, "real-tsrs");
    writeFileSync(real, "");
    chmodSync(real, 0o755);
    expect(() => requireTypeChecker(real)).not.toThrow();
  });
});

describe("typeErrorRefusal", () => {
  it("refuses a run in which every program fails, however small", () => {
    expect(typeErrorRefusal(1, 1)).toMatch(/invalid run: 1 of 1/);
    expect(typeErrorRefusal(300, 300)).toMatch(/300 of 300/);
  });
  it("refuses more than the share and lets a few through", () => {
    expect(TYPE_ERROR_REFUSAL_SHARE).toBe(0.5);
    expect(typeErrorRefusal(151, 300)).toMatch(/invalid run/);
    expect(typeErrorRefusal(150, 300)).toBeNull();
    expect(typeErrorRefusal(5, 300)).toBeNull();
    expect(typeErrorRefusal(0, 300)).toBeNull();
    expect(typeErrorRefusal(0, 0)).toBeNull();
  });
  it("names the first error when it has one", () => {
    expect(typeErrorRefusal(2, 2, { first: "type checker not found: /x" })).toMatch(/first: type checker not found/);
  });
});

describe("every entry point refuses a missing type checker with exit 2", () => {
  const missing = join(scratch, "no-such-tsrs");
  for (const script of ["run.mjs", "matrix.mjs", "equivalence.mjs", "neutrality.mjs", "disposal-check.mjs", "packages-check.mjs"]) {
    it(script, () => {
      const out = join(scratch, `out-${script}`);
      const result = spawnSync(process.execPath, [join(here, script), "--out", out, "--tsc", missing, "--count", "1"], { encoding: "utf8", env: { ...process.env, SOLID_CHECKER_TSRS_BIN: "" } });
      expect(result.status).toBe(2);
      expect(result.stderr).toMatch(/type checker not found/);
      expect(existsSync(out)).toBe(false);
    });
  }
});
