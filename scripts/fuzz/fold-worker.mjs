// A grid's role-log fold and case pruning, off the run's critical path (focus.mjs): a recall grid's log runs to gigabytes and its
// cases/ to a directory per program, and folding and deleting them on the orchestrator's one thread left the VM's cores idle for
// minutes between codes (full-8, 2026-11-02). The next grid starts while this worker folds and prunes the last one.
import { existsSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parentPort, workerData } from "node:worker_threads";
import { foldGrids, summaryLine } from "./roles.mjs";

const { dir, roles, against, pruneCases } = workerData;
let report = null;
try {
  if (roles && existsSync(join(dir, "programs.jsonl"))) {
    report = foldGrids([dir], { against });
    writeFileSync(join(dir, "roles.json"), `${JSON.stringify(report, null, 2)}\n`);
  }
  if (pruneCases) {
    for (const name of ["cases", "chrome-cases"]) rmSync(join(dir, name), { recursive: true, force: true });
    // The folded log is in roles.json; the raw one runs to about 23 KB a program.
    if (roles && report) rmSync(roles, { force: true });
  }
  parentPort.postMessage({ folded: Boolean(report), summary: report ? summaryLine(report) : null, notInInventory: report ? report.notInInventory.slice(0, 5) : [] });
} catch (error) {
  parentPort.postMessage({ folded: false, error: String(error?.stack ?? error).slice(0, 400) });
}
