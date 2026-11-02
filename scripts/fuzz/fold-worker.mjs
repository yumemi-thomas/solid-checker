// A grid's role-log fold and case pruning, off the run's critical path (focus.mjs): a recall grid's log runs to gigabytes and its
// cases/ to a directory per program, and folding and deleting them on the orchestrator's one thread left the VM's cores idle for
// minutes between codes (full-8, 2026-11-02). The next grid starts while this worker folds and prunes the last one.
import { existsSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parentPort, workerData } from "node:worker_threads";
import { addDecisions } from "./cost.mjs";
import { foldGrids, summaryLine } from "./roles.mjs";
import { SCRATCH_DIRS, removeLinked } from "./scratch.mjs";

const { dir, roles, against, pruneCases } = workerData;
let report = null;
// The fold's and the pruning's wall times, for focus.mjs's perf line (perf.mjs).
let foldMs = null;
let pruneMs = null;
try {
  if (roles && existsSync(join(dir, "programs.jsonl"))) {
    const started = performance.now();
    const perProgram = new Map();
    report = foldGrids([dir], { against, perProgram });
    writeFileSync(join(dir, "roles.json"), `${JSON.stringify(report, null, 2)}\n`);
    // Each program's decisions into its cost line (cost.mjs), when the grid wrote them.
    addDecisions(dir, perProgram);
    foldMs = Math.round(performance.now() - started);
  }
  if (pruneCases) {
    const started = performance.now();
    // With `--scratch` these are links (scratch.mjs): the scratch copies go with them.
    for (const name of SCRATCH_DIRS) removeLinked(join(dir, name));
    // The folded log is in roles.json; the raw one runs to about 23 KB a program.
    if (roles && report) removeLinked(roles);
    pruneMs = Math.round(performance.now() - started);
  }
  parentPort.postMessage({ folded: Boolean(report), summary: report ? summaryLine(report) : null, notInInventory: report ? report.notInInventory.slice(0, 5) : [], foldMs, pruneMs });
} catch (error) {
  parentPort.postMessage({ folded: false, error: String(error?.stack ?? error).slice(0, 400), foldMs, pruneMs });
}
