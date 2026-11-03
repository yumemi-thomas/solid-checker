import { existsSync, lstatSync, mkdirSync, mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { SCRATCH_DIRS, freshDir, linkScratch, removeLinked } from "./scratch.mjs";

const made = [];
const temp = () => { const dir = mkdtempSync(join(tmpdir(), "scratch-test-")); made.push(dir); return dir; };
afterEach(() => { for (const dir of made.splice(0)) rmSync(dir, { recursive: true, force: true }); });

describe("scratch directories", () => {
  it("links a grid's cases into the scratch directory, at their usual paths", () => {
    const root = temp();
    const grid = join(root, "runs", "mx-recall", "CODE");
    const scratch = join(root, "ram", "mx-recall", "CODE");
    linkScratch(grid, scratch);
    for (const name of SCRATCH_DIRS) {
      expect(lstatSync(join(grid, name)).isSymbolicLink()).toBe(true);
      expect(realpathSync(join(grid, name))).toBe(realpathSync(join(scratch, name)));
    }
    // What was there is replaced: a grid starts from empty cases.
    mkdirSync(join(grid, "cases", "old"));
    linkScratch(grid, scratch);
    expect(existsSync(join(grid, "cases", "old"))).toBe(false);
    mkdirSync(join(grid, "cases", "p1"));
    expect(existsSync(join(scratch, "cases", "p1"))).toBe(true);
  });

  it("freshDir empties a link's target and keeps the link; a plain directory is recreated", () => {
    const root = temp();
    linkScratch(join(root, "grid"), join(root, "ram"));
    mkdirSync(join(root, "grid", "chrome-cases", "p1"));
    freshDir(join(root, "grid", "chrome-cases"));
    expect(lstatSync(join(root, "grid", "chrome-cases")).isSymbolicLink()).toBe(true);
    expect(existsSync(join(root, "ram", "chrome-cases", "p1"))).toBe(false);
    mkdirSync(join(root, "plain", "x"), { recursive: true });
    freshDir(join(root, "plain"));
    expect(existsSync(join(root, "plain"))).toBe(true);
    expect(existsSync(join(root, "plain", "x"))).toBe(false);
  });

  it("removeLinked deletes a link and its target, a plain path, and a dangling link", () => {
    const root = temp();
    linkScratch(join(root, "grid"), join(root, "ram"));
    removeLinked(join(root, "grid", "cases"));
    expect(existsSync(join(root, "ram", "cases"))).toBe(false);
    rmSync(join(root, "ram", "chrome-cases"), { recursive: true });
    removeLinked(join(root, "grid", "chrome-cases"));
    expect(() => lstatSync(join(root, "grid", "chrome-cases"))).toThrow();
    mkdirSync(join(root, "plain", "x"), { recursive: true });
    removeLinked(join(root, "plain"));
    expect(existsSync(join(root, "plain"))).toBe(false);
  });
});
