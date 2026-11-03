import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { readGrid, runGridProcess, unitProblems } from "./focus.mjs";
import { PARTIAL, partialRows, readPartial, stopReason } from "./partial.mjs";

const here = dirname(fileURLToPath(import.meta.url));

describe("partial rows", () => {
  it("appends a chunk's rows, drops a half-written last line, and is gone once the grid finishes", () => {
    const out = mkdtempSync(join(tmpdir(), "fuzz-partial-"));
    writeFileSync(join(out, PARTIAL), "{\"id\":\"old\"}\n");
    const partial = partialRows(out, { project: (row) => ({ ...row, site: undefined }) });
    expect(readPartial(out)).toBeNull();
    partial.append([{ id: "a", outcome: "agree", site: { line: 1 } }]);
    partial.append([]);
    partial.append([{ id: "b", outcome: "harness" }]);
    writeFileSync(join(out, PARTIAL), `${readFileSync(join(out, PARTIAL), "utf8")}{"id":"c","outc`);
    expect(readPartial(out)).toEqual([{ id: "a", outcome: "agree" }, { id: "b", outcome: "harness" }]);
    partial.finish();
    expect(existsSync(join(out, PARTIAL))).toBe(false);
  });

  it("finds why a grid stopped: its own last word, else Node's uncaught error, else the last line", () => {
    expect(stopReason("matrix: 150/300 programs\nmatrix: stopped: the executor gave no answer to run within 1200 s\n")).toBe("matrix: stopped: the executor gave no answer to run within 1200 s");
    expect(stopReason("legal: 150/300\nnode:events:497\n      throw er;\nError: write EPIPE\n    at afterWriteDispatched\n")).toBe("Error: write EPIPE");
    expect(stopReason("legal: 150/300\n")).toBe("legal: 150/300");
    expect(stopReason("")).toBe("an empty log");
  });
});

describe("a grid process whose host dies", () => {
  // The mx-recall unit of 2026-11-03: the Vite helper died 112,050 programs in and the unit printed `0 programs {}`. Here a grid
  // judges two chunks, then its host's stream fails with an EPIPE nothing handles.
  const grid = (dir, { guard }) => {
    const script = join(dir, "grid.mjs");
    writeFileSync(script, `
      import { EventEmitter } from "node:events";
      import { guardGridExit, partialRows } from ${JSON.stringify(join(here, "partial.mjs"))};
      ${guard ? 'guardGridExit("matrix");' : ""}
      const partial = partialRows(process.argv[2]);
      partial.append([{ id: "p1", outcome: "agree" }, { id: "p2", outcome: "false-clean", detail: { line: 3 } }]);
      console.error("matrix: 2/4 programs");
      partial.append([{ id: "p3", outcome: "agree" }]);
      setTimeout(() => { const error = Object.assign(new Error("write EPIPE"), { code: "EPIPE" }); new EventEmitter().emit("error", error); }, 10);
      setInterval(() => {}, 1000);
    `);
    return script;
  };

  for (const guard of [true, false]) {
    it(`is a harness failure with its reason and the rows judged before it died${guard ? "" : " (unguarded: Node's own crash)"}`, () => {
      const dir = mkdtempSync(join(tmpdir(), "fuzz-host-death-"));
      const out = join(dir, "mx-recall");
      mkdirSync(out);
      const logPath = `${out}.log`;
      const run = runGridProcess(grid(dir, { guard }), [out], { logPath, timeoutMs: 20_000, runtime: "node" });
      expect(run.status).toBe(guard ? 2 : 1);
      const read = readGrid(out);
      expect(read.partial).toBe(true);
      expect(read.outcomes).toEqual({ agree: 2, "false-clean": 1 });
      expect(read.flagged.map((row) => row.id)).toEqual(["p2"]);
      const log = readFileSync(logPath, "utf8");
      const problems = unitProblems("mx-recall", { run, grid: read, total: 3, log, logPath, timeoutMinutes: 120 });
      expect(problems).toHaveLength(2);
      expect(problems[0]).toMatch(/^mx-recall: exited [12] /);
      expect(problems[1]).toMatch(/^mx-recall: harness: the grid process stopped before writing its rows \(3 judged before it stopped, kept in programs\.partial\.jsonl\): .*EPIPE/);
    }, 30_000);
  }

  it("a grid that left no rows at all says so; a complete grid with rows needs no look", () => {
    const dir = mkdtempSync(join(tmpdir(), "fuzz-host-death-"));
    const none = readGrid(join(dir, "never-started"));
    expect(none.partial).toBe(false);
    expect(unitProblems("lg-base", { run: { status: 2 }, grid: none, total: 0, log: "legal: stopped: the executor did not start: x\n", logPath: "l.log", timeoutMinutes: 120 })[1])
      .toBe("lg-base: harness: the grid process stopped before writing its rows (none judged): legal: stopped: the executor did not start: x");
    writeFileSync(join(dir, "programs.jsonl"), "{\"id\":\"a\",\"outcome\":\"clean\"}\n");
    expect(unitProblems("lg-base", { run: { status: 0 }, grid: readGrid(dir), total: 1, log: "", logPath: "l.log", timeoutMinutes: 120 })).toEqual([]);
    expect(unitProblems("lg-base", { run: { status: 0 }, grid: { outcomes: {} }, total: 0, log: "", logPath: "l.log", timeoutMinutes: 120 })).toEqual(["lg-base: no program was judged (l.log)"]);
  });
});
