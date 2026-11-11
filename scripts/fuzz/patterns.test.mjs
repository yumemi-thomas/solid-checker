import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { AssumedReportError } from "../lib/assumed.mjs";
import { TAINTED, cleanReport, fakeChecker, finding } from "../lib/assumed-fixtures.mjs";
import { runChecker } from "./patterns.mjs";

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
