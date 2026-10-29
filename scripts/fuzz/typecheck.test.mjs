import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, describe, expect, it } from "vitest";
import { DEFAULT_TSC, TYPE_ERROR_REFUSAL_SHARE, createTypecheckBatcher, harnessRefusal, requireTypeChecker, resolveTsc, typeErrorRefusal, typecheckBatch } from "./typecheck.mjs";
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

describe("harnessRefusal", () => {
  it("refuses a run the harness could not judge, more than half of it", () => {
    expect(harnessRefusal(184, 184, { first: "could not start" })).toMatch(/invalid run: the harness could not judge 184 of 184.*could not start/);
    expect(harnessRefusal(3, 5)).toMatch(/3 of 5/);
    expect(harnessRefusal(1, 2)).toBeNull();
    expect(harnessRefusal(0, 0)).toBeNull();
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

describe("typecheckBatch", () => {
  // A fake tsrs: with `-b` it prints the lines of $FAKE_BATCH (paths relative to its cwd, the first project) and exits $FAKE_STATUS;
  // with `-p` it prints $FAKE_SINGLE_<n> for project n. It also writes the build-info file `-b` writes.
  const fake = join(scratch, "fake-tsrs");
  writeFileSync(fake, `#!/bin/sh
if [ "$1" = "-b" ]; then
  shift; shift
  for p in "$@"; do case "$p" in *tsconfig.json) : > "$(dirname "$p")/tsconfig.tsbuildinfo";; esac; done
  printf "%b" "$FAKE_BATCH"; exit \${FAKE_STATUS:-0}
fi
printf "single %s error TS1111: alone\\n" "$(basename "$(pwd)")"; exit 1
`);
  chmodSync(fake, 0o755);
  const dirs = ["p1", "p2", "p3"].map((name) => { const dir = join(scratch, "batch", name); mkdirSync(dir, { recursive: true }); writeFileSync(join(dir, "tsconfig.json"), "{}"); return dir; });
  const run = (batch, status = 0) => { process.env.FAKE_BATCH = batch; process.env.FAKE_STATUS = String(status); return typecheckBatch(dirs, { tsc: fake }); };

  it("answers each project as a lone -p run prints it, and removes the build-info files", async () => {
    const answers = await run("App.tsx(1,2): error TS2322: in p1\\n../p3/App.tsx(3,4): error TS2345: in p3\\nFound 2 errors.\\n", 1);
    expect(answers.get(dirs[0])).toEqual({ ok: false, errors: ["App.tsx(1,2): error TS2322: in p1"] });
    expect(answers.get(dirs[1])).toEqual({ ok: true, errors: [] });
    expect(answers.get(dirs[2])).toEqual({ ok: false, errors: ["App.tsx(3,4): error TS2345: in p3"] });
    expect(dirs.some((dir) => existsSync(join(dir, "tsconfig.tsbuildinfo")))).toBe(false);
  });

  it("answers nothing for an error it cannot place, or a failed run with no diagnostic", async () => {
    expect(await run("error TS2318: Cannot find global type 'Array'.\\n", 1)).toBeNull();
    expect(await run("../elsewhere/x.ts(1,1): error TS1005: outside\\n", 1)).toBeNull();
    expect(await run("", 3)).toBeNull();
  });

  it("the batcher checks each project alone when the batch answers nothing", async () => {
    process.env.FAKE_BATCH = "error TS2318: unplaced\\n"; process.env.FAKE_STATUS = "1";
    const check = createTypecheckBatcher({ size: 3 });
    const answers = await Promise.all(dirs.map((dir) => check(dir, { tsc: fake })));
    expect(answers.map((answer) => answer.errors)).toEqual([["single p1 error TS1111: alone"], ["single p2 error TS1111: alone"], ["single p3 error TS1111: alone"]]);
  });
});
