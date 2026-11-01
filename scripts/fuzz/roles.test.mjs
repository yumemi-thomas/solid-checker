import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { parse } from "./focus.mjs";
import { armInventory, foldDecisions, foldGrids, foldRoles, gridLog, programOf, readRoleLog, slicesYield, summaryLine } from "./roles.mjs";

const work = "/i/.solid-checker-fuzz/run-1-ab";
const decision = (program, s, arm, extra = {}) => ({ k: "read", f: `${work}/${program}/App.tsx`, s, e: s + 3, role: "DeferredCallback", arm, arms: [arm], default: false, silent: true, ...extra });

describe("the arm inventory", () => {
  it("lists every tagged arm of the checker's role and owner sources once, defaults marked", () => {
    const arms = armInventory();
    expect(arms.size).toBeGreaterThan(100);
    expect(arms.get("callee_roles.allowed-unplaced-nested")).toEqual({ default: true, file: "execution_role/callee_roles.rs" });
    // ADR 0535: an assigned member answers Unknown now, a fact, not a default.
    expect(arms.get("callee_roles.assigned-member-function")).toEqual({ default: false, file: "execution_role/callee_roles.rs" });
    expect(arms.get("owner_emission.context-0")?.default).toBe(true);
    expect(arms.get("callee_roles.inside-component")?.default).toBe(false);
    for (const [id, { file }] of arms) expect(id.startsWith(`${file.split("/").pop().replace(/\.rs$/, "")}.`)).toBe(true);
  });

  it("refuses an id tagged twice or under another file's stem", () => {
    const root = mkdtempSync(join(tmpdir(), "fuzz-roles-"));
    mkdirSync(join(root, "owners"));
    writeFileSync(join(root, "owners", "a.rs"), 'role_arm!("a.x", r); arm_id!(default\n "a.y"); role_arm!("a.x", r);');
    expect(() => armInventory(root)).toThrow(/a\.x is tagged twice/);
    writeFileSync(join(root, "owners", "a.rs"), 'role_arm!("b.x", r);');
    expect(() => armInventory(root)).toThrow(/starts with its file's stem/);
    writeFileSync(join(root, "owners", "a.rs"), 'role_arm!(\n    default "a.x",\n    r\n); arm_id!("a.y")');
    expect([...armInventory(root)]).toEqual([["a.x", { default: true, file: "owners/a.rs" }], ["a.y", { default: false, file: "owners/a.rs" }]]);
  });
});

describe("the fold", () => {
  it("maps a checker work path to its program", () => {
    expect(programOf(`${work}/p-1/src/App.tsx`)).toEqual({ program: "p-1", file: "src/App.tsx" });
    expect(programOf("/elsewhere/App.tsx")).toEqual({ program: null, file: "/elsewhere/App.tsx" });
  });

  it("counts a decision once per program and site, and the silent answers of default arms", () => {
    const inventory = new Map([["f.a", { default: false }], ["f.d", { default: true }], ["f.never", { default: false }]]);
    const lines = [
      decision("p1", 1, "f.d", { arms: ["f.a", "f.d"] }),
      decision("p1", 1, "f.d", { arms: ["f.a", "f.d"] }), // the same program checked twice
      decision("p2", 1, "f.d"),
      decision("p2", 9, "f.d", { role: "Unknown", silent: false }),
      decision("p2", 20, "f.a", { k: "write", role: "TrackedJsx", silent: false }),
      { k: "owner", rule: "effect", f: `${work}/p1/App.tsx`, s: 5, e: 9, ctx: 0, arm: "f.d", arms: ["f.d"], default: true, silent: true },
      { k: "unframed", role: "Unknown", arm: "f.u", arms: ["f.u"], default: false },
    ];
    const folded = foldDecisions(lines, inventory);
    expect(folded.decisions).toBe(5);
    expect(folded.unframed).toBe(1);
    expect(folded.arms["f.d"]).toEqual({ passed: 4, answered: 4, programs: 2, silent: 3, roles: { DeferredCallback: 2, Unknown: 1, "ctx 0": 1 }, kinds: { read: 3, "owner:effect": 1 }, default: true });
    expect(folded.arms["f.a"]).toMatchObject({ passed: 2, answered: 1, kinds: { write: 1 } });
    expect(folded.arms["f.u"]).toMatchObject({ passed: 0, answered: 0, unframed: 1 });
    expect(folded.neverHit).toEqual(["f.never"]);
    expect(folded.notInInventory).toEqual(["f.u"]);
    expect(folded.silentByDefault).toEqual({ decisions: 3, byArm: { "f.d": 3 } });
  });

  it("gives findings per 1,000 programs, fast and Chrome-confirmed", () => {
    const rows = [{ outcome: "agree" }, { outcome: "false-clean", chrome: { status: "same" } }, { outcome: "false-positive" }, { outcome: "clean" }];
    expect(slicesYield(rows)).toEqual({ programs: 4, candidates: 2, confirmed: 1, candidatesPerThousand: 500, confirmedPerThousand: 250 });
    expect(slicesYield([])).toMatchObject({ programs: 0, confirmedPerThousand: 0 });
  });

  it("names the arms first hit against an earlier run, none without one", () => {
    const inventory = new Map([["f.a", { default: false }], ["f.b", { default: false }]]);
    const lines = [decision("p", 1, "f.a"), decision("p", 2, "f.b")];
    const report = foldRoles({ lines, rows: [{ outcome: "agree" }], inventory, against: { hit: ["f.a"], path: "/old/roles.json" } });
    expect(report.saturation.growth).toEqual({ against: "/old/roles.json", newArms: 1, arms: ["f.b"] });
    expect(report.armsHit).toBe(2);
    expect(foldRoles({ lines, rows: [], inventory }).saturation.growth).toEqual({ against: null, newArms: null, arms: null });
    expect(summaryLine(report)).toMatch(/^roles: 2\/2 arms hit \(0 never\), 2 decisions; .*new arms 1; silent by default 0$/);
  });

  it("reads a grid's `<dir>.roles.jsonl` or `<dir>.roles/` (one file per process), counts malformed lines, and refuses a grid with no log", () => {
    const root = mkdtempSync(join(tmpdir(), "fuzz-roles-"));
    const grid = join(root, "mx-base");
    mkdirSync(grid);
    mkdirSync(`${grid}.roles`);
    writeFileSync(join(grid, "programs.jsonl"), `${JSON.stringify({ id: "p1", outcome: "false-clean" })}\n${JSON.stringify({ id: "p2", outcome: "agree" })}\n`);
    writeFileSync(join(`${grid}.roles`, "roles-1.jsonl"), `${JSON.stringify(decision("p1", 1, "f.a"))}\n{"k":"read",\n`);
    writeFileSync(join(`${grid}.roles`, "roles-2.jsonl"), `${JSON.stringify(decision("p2", 1, "f.a"))}\n`);
    writeFileSync(join(`${grid}.roles`, "ignored.txt"), "not a log\n");
    expect(readRoleLog(`${grid}.roles`)).toMatchObject({ malformed: 1, missing: false });
    const report = foldGrids([grid], { inventory: new Map([["f.a", { default: false }]]) });
    expect(report).toMatchObject({ decisions: 2, malformed: 1, armsHit: 1, armsKnown: 1, neverHit: [] });
    expect(report.saturation.yield).toMatchObject({ programs: 2, candidates: 1, confirmed: 0 });
    expect(() => foldGrids([join(root, "missing")])).toThrow(/no role log/);
    expect(gridLog(grid)).toBe(`${grid}.roles`);
    writeFileSync(`${grid}.roles.jsonl`, `${JSON.stringify(decision("p1", 1, "f.a"))}\n${JSON.stringify(decision("p2", 4, "f.a"))}\n`);
    expect(gridLog(grid)).toBe(`${grid}.roles.jsonl`);
    expect(foldGrids([grid], { inventory: new Map([["f.a", { default: false }]]) })).toMatchObject({ decisions: 2, malformed: 0 });
  });
});

describe("focus --role-log", () => {
  it("takes the flag, and --roles-against only with it", () => {
    expect(parse(["--codes", "A", "--role-log", "--roles-against", "/r.json"])).toMatchObject({ roleLog: true, rolesAgainst: "/r.json" });
    expect(() => parse(["--codes", "A", "--roles-against", "/r.json"])).toThrow(/needs --role-log/);
  });
});

describe("the run-wide merge", () => {
  it("adds the grids' roles.json reports, so pruned raw logs are not needed", async () => {
    const { mkdtempSync, mkdirSync, writeFileSync } = await import("node:fs");
    const { tmpdir } = await import("node:os");
    const { join } = await import("node:path");
    const { mergeGridReports } = await import("./roles.mjs");
    const root = mkdtempSync(join(tmpdir(), "roles-merge-"));
    const grid = (name, arm, passed, outcome) => {
      const dir = join(root, name);
      mkdirSync(dir);
      writeFileSync(join(dir, "programs.jsonl"), `${JSON.stringify({ id: name, outcome })}\n`);
      writeFileSync(join(dir, "roles.json"), JSON.stringify({ decisions: passed, arms: { [arm]: { passed, answered: passed, programs: 1, silent: 0, roles: { Unknown: passed }, kinds: { read: passed } } }, silentByDefault: { [arm]: 1 } }));
      return dir;
    };
    const report = mergeGridReports([grid("a", "f.a", 2, "agree"), grid("b", "f.a", 3, "false-clean"), grid("c", "f.b", 1, "agree")], { inventory: new Map([["f.a", { default: true }], ["f.b", { default: false }], ["f.c", { default: false }]]) });
    expect(report).toMatchObject({ decisions: 6, armsHit: 2, armsKnown: 3, neverHit: ["f.c"] });
    expect(report.arms["f.a"]).toMatchObject({ passed: 5, programs: 2, roles: { Unknown: 5 } });
    expect(report.saturation.silentByDefault).toBe(3);
    expect(report.saturation.yield).toMatchObject({ programs: 3, candidates: 1 });
  });
});
