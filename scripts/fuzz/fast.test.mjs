import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { PINNED_SOLID, installedVersions, pinnedProblems, runFast } from "./fast.mjs";

const all = Object.fromEntries(["solid-js", "@solidjs/web", "@solidjs/signals", "@solidjs/diagnostics", "@solidjs/compiler", "@solidjs/babel-plugin"].map((name) => [name, PINNED_SOLID]));

describe("pinned install", () => {
  it("pins rc.17 (invariant 7: never a newer prerelease)", () => {
    expect(PINNED_SOLID).toBe("2.0.0-rc.17");
    expect(pinnedProblems(all)).toEqual([]);
  });
  it("names every package that is missing or not the pin", () => {
    const problems = pinnedProblems({ ...all, "@solidjs/compiler": "2.0.0-rc.18", "solid-js": undefined });
    expect(problems).toEqual(["solid-js is not installed", "@solidjs/compiler is 2.0.0-rc.18, not 2.0.0-rc.17"]);
  });
  it("reads versions from node_modules and refuses a runFast on an install that is not the pin", async () => {
    const install = mkdtempSync(join(tmpdir(), "fuzz-fast-"));
    mkdirSync(join(install, "node_modules/solid-js"), { recursive: true });
    writeFileSync(join(install, "node_modules/solid-js/package.json"), JSON.stringify({ version: "2.0.0-rc.18" }));
    expect(installedVersions(install)["solid-js"]).toBe("2.0.0-rc.18");
    await expect(runFast([], { install })).rejects.toThrow(/pinned install.*solid-js is 2.0.0-rc.18/);
  });
  it("refuses an unsafe case id before writing anything", async () => {
    await expect(runFast([{ id: "../x", dir: "/tmp" }], { install: "/nonexistent" })).rejects.toThrow(/unsafe case id/);
  });
});
