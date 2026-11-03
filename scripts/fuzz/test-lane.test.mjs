import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { manualCommand, requirements, unmet, unmetMessage } from "./provision.mjs";
import { TSRS_ONLY, judgeSkips, skippedIn } from "./test-lane.mjs";

const skip = (file, name = "x") => ({ file: `/repo/scripts/fuzz/${file}`, name });

describe("the lane's skip policy", () => {
  it("allows the tsrs-only files to skip without a tsrs binary and nothing else", () => {
    const tests = [skip("checker.test.mjs"), skip("rewrite.test.mjs"), skip("tsc-overlap.test.mjs"), skip("ssr.test.mjs"), skip("fast-daemon.test.mjs")];
    const { allowed, failed } = judgeSkips(tests, { tsrsPresent: false });
    expect(allowed.map((test) => test.file.split("/").pop())).toEqual(TSRS_ONLY);
    expect(failed.map((test) => test.file.split("/").pop())).toEqual(["ssr.test.mjs", "fast-daemon.test.mjs"]);
  });

  it("fails every skip when tsrs is present: it must run", () => {
    const { allowed, failed } = judgeSkips([skip("checker.test.mjs"), skip("rewrite.test.mjs")], { tsrsPresent: true });
    expect(allowed).toEqual([]);
    expect(failed).toHaveLength(2);
  });

  it("reads the skipped and todo tests of vitest's JSON report, not the passed or failed", () => {
    const report = { testResults: [{ name: "/r/a.test.mjs", assertionResults: [{ status: "passed", fullName: "p" }, { status: "failed", fullName: "f" }, { status: "skipped", fullName: "s" }, { status: "todo", fullName: "t" }] }] };
    expect(skippedIn(report)).toEqual([{ file: "/r/a.test.mjs", name: "s" }, { file: "/r/a.test.mjs", name: "t" }]);
  });

  it("names the skipIf'd files in the source, so the allow-list cannot drift from what skips", async () => {
    const { readFileSync, readdirSync } = await import("node:fs");
    const here = new URL(".", import.meta.url).pathname;
    const withTsrs = readdirSync(here).filter((name) => name.endsWith(".test.mjs") && /skipIf\(!(typed|typedHere|installed)\)/.test(readFileSync(join(here, name), "utf8")) && /resolveTsc|DEFAULT_TSC|TSC\b/.test(readFileSync(join(here, name), "utf8")));
    expect(withTsrs.sort()).toEqual([...TSRS_ONLY].sort());
  });
});

describe("provisioning", () => {
  const root = mkdtempSync(join(tmpdir(), "fuzz-provision-"));
  const empty = (name) => { const dir = join(root, name); mkdirSync(dir, { recursive: true }); return dir; };

  it("reports a missing install loudly, with the command that provisions it", () => {
    const list = requirements({ fastInstall: empty("fast"), oracleInstall: empty("oracle") });
    const found = unmet(list);
    // The headless shell reports nothing of its own while the oracle install it is named by is missing: one cause, one line.
    expect(found.map(({ requirement }) => requirement.name)).toEqual(["fuzz fast-path install", "runtime oracle install"]);
    const message = unmetMessage(found);
    expect(message).toMatch(/solid-js is not installed/);
    expect(message).toMatch(/provision: cd .*fast && npm ci/);
    expect(message).toMatch(/bun scripts\/fuzz\/provision\.mjs/);
  });

  it("asks for the pinned browser once the oracle install names it", () => {
    const oracle = empty("oracle2");
    mkdirSync(join(oracle, "node_modules/playwright-core"), { recursive: true });
    writeFileSync(join(oracle, "node_modules/playwright-core/browsers.json"), JSON.stringify({ browsers: [{ name: "chromium-headless-shell", revision: "1", browserVersion: "1" }] }));
    writeFileSync(join(oracle, "node_modules/playwright-core/package.json"), JSON.stringify({ version: "1.0.0" }));
    const shell = requirements({ fastInstall: empty("fast2"), oracleInstall: oracle, browser: () => { throw new Error("not installed in /cache"); } }).at(-1);
    expect(shell.problems()).toEqual(["not installed in /cache"]);
    expect(manualCommand(shell)).toMatch(/npx playwright install chromium-headless-shell$/);
    expect(requirements({ fastInstall: empty("fast3"), oracleInstall: oracle, browser: () => ({}) }).at(-1).problems()).toEqual([]);
    rmSync(root, { recursive: true, force: true });
  });
});
