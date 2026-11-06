import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { parse } from "./focus.mjs";
import { armInventory, foldDecisions, foldGrids, foldRoles, gridLog, programOf, readRoleLog, resolvedLine, slicesYield, summaryLine } from "./roles.mjs";

const work = "/i/.solid-checker-fuzz/run-1-ab";
const decision = (program, s, arm, extra = {}) => ({ k: "read", f: `${work}/${program}/App.tsx`, s, e: s + 3, role: "DeferredCallback", arm, arms: [arm], default: false, silent: true, ...extra });

describe("the arm inventory", () => {
  it("lists every tagged arm of the checker's role and owner sources once, defaults marked", () => {
    const arms = armInventory();
    expect(arms.size).toBeGreaterThan(100);
    expect(arms.get("callee_roles.inheriting-tracked-root")).toEqual({ default: true, file: "execution_role/callee_roles.rs" });
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

  it("maps the Rust executor's slot to the program it holds, a twin's slot to `<id>-run`, Chrome's staging to its case", () => {
    expect(programOf(`${work}/slots/3/lg-x-closure/App.tsx`)).toEqual({ program: "lg-x-closure", file: "App.tsx" });
    expect(programOf(`${work}/slots/3-run/lg-x-closure/App.tsx`)).toEqual({ program: "lg-x-closure-run", file: "App.tsx" });
    expect(programOf(`${work}/slots/12/a%2Fb%20c/sub/helpers.ts`)).toEqual({ program: "a/b c", file: "sub/helpers.ts" });
    expect(programOf("/i/.solid-checker-runtime-oracle/lg-x-iife/App.tsx")).toEqual({ program: "lg-x-iife", file: "App.tsx" });
    // A log from before the slots were named: one shared program, kept foldable as it was, and marked.
    expect(programOf(`${work}/slots/slot-3/App.tsx`)).toEqual({ program: "slots", file: "slot-3/App.tsx", legacySlot: true });
    expect(programOf(`${work}/slots/slot-3-run/App.tsx`)).toMatchObject({ legacySlot: true });
  });

  it("keeps two programs that shared a slot apart, and counts an old log's shared slot as unattributed", () => {
    const at = (dir, s, arm) => ({ k: "read", f: `${work}/${dir}/App.tsx`, s, e: s + 3, role: "Unknown", arm, arms: [arm] });
    // Two programs ran on slot 0 one after the other, with a decision at the same span: two decisions, two programs.
    const named = foldDecisions([at("slots/0/p1", 1, "f.a"), at("slots/0/p2", 1, "f.a"), at("slots/1/p1", 1, "f.a")], null);
    expect(named).toMatchObject({ decisions: 2, unattributed: 0 });
    expect(named.arms["f.a"].programs).toBe(2);
    // The same programs logged by the old executor: one slot directory, so the decisions fold as one and name no program.
    const legacy = foldDecisions([at("slots/slot-0", 1, "f.a"), at("slots/slot-0", 1, "f.a"), at("slots/slot-1", 1, "f.a")], null);
    expect(legacy).toMatchObject({ decisions: 2, unattributed: 2 });
    expect(legacy.arms["f.a"].programs).toBe(0);
    // A warm slot (ADR 0543) keeps one directory for every program; each path line names its program (`g`), and the fold is the
    // one-shot fold of the same decisions.
    const tagged = (program, s, arm, slot = "0") => ({ ...at(`slots/${slot}/program`, s, arm), g: program });
    const warm = foldDecisions([tagged("p1", 1, "f.a"), tagged("p2", 1, "f.a"), tagged("p1", 1, "f.a", "1")], null);
    expect(warm).toEqual(named);
    expect(summaryLine(foldRoles({ lines: [at("slots/slot-0", 1, "f.a")], rows: [], inventory: null }))).toMatch(/1 decisions \(1 unattributed\);/);
  });

  it("takes a path line's program over the program its path names, keeping a twin slot's -run", () => {
    expect(programOf(`${work}/slots/3/program/App.tsx`, "lg-x")).toEqual({ program: "lg-x", file: "App.tsx" });
    expect(programOf(`${work}/slots/3-run/program/sub/a.ts`, "lg-x")).toEqual({ program: "lg-x-run", file: "sub/a.ts" });
    expect(programOf(`${work}/p-1/App.tsx`, "lg-y")).toEqual({ program: "lg-y", file: "App.tsx" });
    expect(programOf(`${work}/slots/3/lg-x/App.tsx`, undefined)).toEqual({ program: "lg-x", file: "App.tsx" });
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

describe("path lines", () => {
  const inventory = new Map([["f.a", { default: false }], ["f.d", { default: true }]]);
  // The lines of two processes as the old format writes them: every line names its file.
  const old = [
    decision("p1", 1, "f.d", { arms: ["f.a", "f.d"] }),
    decision("p2", 1, "f.d"),
    { k: "owner", rule: "effect", f: `${work}/p1/App.tsx`, s: 5, e: 9, ctx: 0, arm: "f.d", arms: ["f.d"], default: true, silent: true },
    { k: "unframed", role: "Unknown", arm: "f.a", arms: ["f.a"], default: false },
    decision("p1", 7, "f.a", { k: "write", f: `${work}/p1/helpers.ts`, role: "TrackedJsx", silent: false }),
    decision("p2", 1, "f.d"),
  ];
  // The same lines as role_log.rs writes them now: a path line per (process, file), then `q` and `p`.
  const pathLine = (q, p, f) => ({ k: "path", q, p, f });
  const named = (line, q, p) => { const { f, ...rest } = line; return { ...rest, q, p }; };
  const current = [
    pathLine(10, 0, `${work}/p1/App.tsx`), named(old[0], 10, 0),
    pathLine(11, 0, `${work}/p2/App.tsx`), named(old[1], 11, 0),
    named(old[2], 10, 0),
    old[3],
    pathLine(10, 1, `${work}/p1/helpers.ts`), named(old[4], 10, 1),
    named(old[5], 11, 0),
  ];
  const text = (lines) => lines.map((line) => JSON.stringify(line)).join("\n") + "\n";

  it("restore each line's file, so the old and the new format fold the same", () => {
    const root = mkdtempSync(join(tmpdir(), "fuzz-roles-paths-"));
    const grids = ["old", "new", "mixed"].map((name) => {
      const grid = join(root, name);
      mkdirSync(grid);
      writeFileSync(join(grid, "programs.jsonl"), `${JSON.stringify({ id: "p1", outcome: "agree" })}\n`);
      return grid;
    });
    writeFileSync(`${grids[0]}.roles.jsonl`, text(old));
    writeFileSync(`${grids[1]}.roles.jsonl`, text(current));
    // A file appended to by a checker of each format.
    writeFileSync(`${grids[2]}.roles.jsonl`, text([...old.slice(0, 4), ...current.slice(6, 8), old[5]]));
    expect(readRoleLog(`${grids[1]}.roles.jsonl`).lines).toEqual(old);
    const [a, b, c] = grids.map((grid) => foldGrids([grid], { inventory }));
    for (const report of [a, b, c]) delete report.sources;
    expect(b).toEqual(a);
    expect(c).toEqual(a);
    expect(a).toMatchObject({ decisions: 4, unframed: 1, malformed: 0 });
  });

  it("keep the processes appending to one file apart, take a reused pid's later path line, and count a line with no path line as malformed", () => {
    const counter = { malformed: 0 };
    const paths = new Map();
    const resolve = (line) => resolvedLine(line, paths, counter);
    expect(resolve(pathLine(10, 0, "/a/App.tsx"))).toBe(null);
    expect(resolve(pathLine(11, 0, "/b/App.tsx"))).toBe(null);
    expect(resolve({ k: "read", q: 11, p: 0, s: 1 })).toEqual({ k: "read", s: 1, f: "/b/App.tsx" });
    expect(resolve({ k: "read", q: 10, p: 0, s: 1 })).toEqual({ k: "read", s: 1, f: "/a/App.tsx" });
    // Process 10 exited; a later process got its pid and numbers its own paths from 0.
    expect(resolve(pathLine(10, 0, "/c/App.tsx"))).toBe(null);
    expect(resolve({ k: "read", q: 10, p: 0, s: 2 })).toEqual({ k: "read", s: 2, f: "/c/App.tsx" });
    expect(resolve({ k: "read", q: 10, p: 1, s: 2 })).toBe(null);
    expect(counter.malformed).toBe(1);
    // A path line that names its program (a warm daemon's) gives it to the lines that name the path.
    expect(resolve({ ...pathLine(12, 7, "/s/0/program/App.tsx"), g: "p9" })).toBe(null);
    expect(resolve({ k: "read", q: 12, p: 7, s: 3 })).toEqual({ k: "read", s: 3, f: "/s/0/program/App.tsx", g: "p9" });
    // An old line and an unframed line pass as they are.
    expect(resolve(old[0])).toBe(old[0]);
    expect(resolve(old[3])).toBe(old[3]);
  });

  it("read a directory's files each with its own table", () => {
    const root = mkdtempSync(join(tmpdir(), "fuzz-roles-paths-"));
    writeFileSync(join(root, "roles-10.jsonl"), text([pathLine(10, 0, `${work}/p1/App.tsx`), named(old[0], 10, 0)]));
    // No path line in this file: process 10's table is the other file's.
    writeFileSync(join(root, "roles-11.jsonl"), text([named(old[1], 10, 0)]));
    expect(readRoleLog(root)).toEqual({ lines: [old[0]], malformed: 1, missing: false });
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

describe("the fold worker", () => {
  it("folds a grid's log into roles.json and prunes its cases and raw log, off the orchestrator's thread", async () => {
    const { existsSync, mkdtempSync, mkdirSync, readFileSync, writeFileSync } = await import("node:fs");
    const { tmpdir } = await import("node:os");
    const { join } = await import("node:path");
    const { Worker } = await import("node:worker_threads");
    const root = mkdtempSync(join(tmpdir(), "fold-worker-"));
    const dir = join(root, "CODE");
    mkdirSync(join(dir, "cases", "p1"), { recursive: true });
    writeFileSync(join(dir, "programs.jsonl"), `${JSON.stringify({ id: "p1", outcome: "agree" })}\n`);
    const roles = `${dir}.roles.jsonl`;
    writeFileSync(roles, `${JSON.stringify({ k: "read", f: `/x/.solid-checker-fuzz/run-1/p1/App.tsx`, s: 1, e: 2, role: "Unknown", arm: "f.a", arms: ["f.a"] })}\n`);
    const message = await new Promise((resolve, reject) => {
      const worker = new Worker(new URL("./fold-worker.mjs", import.meta.url), { workerData: { dir, roles, against: null, pruneCases: true } });
      worker.once("message", resolve);
      worker.once("error", reject);
    });
    expect(message).toMatchObject({ folded: true });
    expect(JSON.parse(readFileSync(join(dir, "roles.json"), "utf8")).decisions).toBe(1);
    expect(existsSync(join(dir, "cases"))).toBe(false);
    expect(existsSync(roles)).toBe(false);
  });
});
