import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { AssumedReportError } from "../lib/assumed.mjs";
import { TAINTED, cleanReport, fakeChecker, finding } from "../lib/assumed-fixtures.mjs";
import { mapStacks, runChecker } from "./patterns.mjs";

// ADR 0950: the pattern lane judges each program's checker report against what Chrome reported. A what-if report is a prediction, so the lane
// stops on it; it is never a "no checker report" row (`no-run`), which a person reads past.
describe("the pattern lane's checker run", () => {
  /** One pattern case in a throwaway directory, `body({ root, cases })`. */
  const withCase = async (body) => {
    const root = mkdtempSync(join(tmpdir(), "patterns-what-if-"));
    try {
      const dir = join(root, "patterns", "report", "case");
      mkdirSync(dir, { recursive: true });
      writeFileSync(join(dir, "App.tsx"), "export const App = () => null;\n");
      mkdirSync(join(root, "check"));
      return await body({ root, cases: [{ id: "report/case", report: "report", dir }] });
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  };

  for (const [name, make] of Object.entries(TAINTED)) {
    it(`rejects with the refusal when the checker prints a what-if report (${name})`, () => withCase(async ({ root, cases }) => {
      const run = runChecker(cases, join(root, "check"), { checker: fakeChecker(root, make()), typefacts: "/tf/solid-typefacts" });
      await expect(run).rejects.toBeInstanceOf(AssumedReportError);
      await expect(run).rejects.toThrow(/^scripts\/fuzz\/patterns\.mjs refuses this checker report: it is tainted by a what-if assumption/);
    }));
  }

  it("reads a default build's report as before, leaving the harness entry's own findings out, and an unreadable one is a case with no report", () => withCase(async ({ root, cases }) => {
    const at = (file, extra = {}) => finding({ primaryLocation: { path: join(root, "check", "report--case", file), startByte: 0, endByte: 1, line: 1, column: 1, endLine: 1 }, ...extra });
    const report = { ...cleanReport(), findings: [at("App.tsx"), at("main.tsx")] };
    const results = await runChecker(cases, join(root, "check"), { checker: fakeChecker(root, report), typefacts: "/tf/solid-typefacts" });
    expect(results.get("report/case")).toEqual([report.findings[0]]);
    const unreadable = join(root, "unreadable");
    writeFileSync(unreadable, "#!/bin/sh\necho not json\n", { mode: 0o755 });
    expect((await runChecker(cases, join(root, "check"), { checker: unreadable, typefacts: "/tf/solid-typefacts" })).get("report/case")).toBeNull();
  }));
});

// The re-review of #222's NO_OWNER_EFFECT follow-up: the lane maps a stack as the grids' Chrome host does (chrome-host.mjs `mapPositions`, ADRs
// 0921 and 0930). A frame at a compiled `spread` call has no source position of its own and stands for the spread its `loc` names; before, the
// lane dropped it, kept only the call that handed the step to a package, and judged the checker's row at the spread a false clean.
describe("the pattern lane's stacks (mapStacks)", () => {
  const SERVED = [
    'import { spread as _$spread } from "/.vite/deps/@solidjs_web.js?v=1";',
    'import { insert as _$insert } from "/.vite/deps/@solidjs_web.js?v=1";',
    '_$spread(_el$3, { title: cb }, false, undefined, "div", "cases/report--case/App.tsx:4:14");',
    "runStep(step);",
  ].join("\n");
  const APP = 'import { runStep } from "fz-step";\nconst cb = () => 1;\nfunction step() {\n  const el = <div {...{ title: cb }} />;\n}\nsetTimeout(() => {\n  runStep(step);\n});\n';
  const server = { moduleGraph: { getModuleByUrl: async () => ({ transformResult: { map: { mappings: "x" }, code: SERVED } }) } };
  // The compiled spread call (served line 3) maps to nothing; the hand-off call (served line 4) maps to `runStep(step)`, 7:3.
  const lookup = { TraceMap: class {}, originalPositionFor: (_, { line }) => (line === 3 ? { line: null, column: null } : { line: 7, column: 2 }), traceSegment: () => null };
  const withCase = async (body) => {
    const root = mkdtempSync(join(tmpdir(), "patterns-stacks-"));
    try {
      const dir = join(root, "report--case");
      mkdirSync(dir, { recursive: true });
      writeFileSync(join(dir, "App.tsx"), APP);
      return await body(dir);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  };
  const frames = [{ url: "/cases/report--case/App.tsx", line: 3, column: 1 }, { url: "/cases/report--case/App.tsx", line: 4, column: 1 }];

  it("keeps a compiled spread's frame, standing for the spread its loc names, above the hand-off call", () => withCase(async (dir) => {
    const stacks = await mapStacks(server, lookup, dir, { sites: [{ code: "NO_OWNER_EFFECT", frames }] });
    const [top, below] = stacks.NO_OWNER_EFFECT[0];
    expect(top).toMatchObject({ file: "App.tsx", line: 4, column: 14, unmapped: true, starts: [{ line: 4, column: 14, compiled: true }] });
    expect(below).toMatchObject({ file: "App.tsx", line: 7, column: 3 });
  }));

  it("places nothing for a served module with no map", () => withCase(async (dir) => {
    const bare = { moduleGraph: { getModuleByUrl: async () => ({ transformResult: { code: SERVED } }) } };
    const stacks = await mapStacks(bare, lookup, dir, { sites: [{ code: "NO_OWNER_EFFECT", frames }] });
    expect(stacks.NO_OWNER_EFFECT).toEqual([[]]);
  }));
});
