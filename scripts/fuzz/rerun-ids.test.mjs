// Run: bun packages/cli/node_modules/vitest/vitest.mjs run --config packages/cli/vitest.config.mjs scripts/fuzz/rerun-ids.test.mjs
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { generateLegal, recallLegalShapes } from "./legal.mjs";
import { rerunCommand, selectRerun } from "./rerun-ids.mjs";

describe("rerun-ids.mjs", () => {
  it("collects a split run's harness rows per grid and code, with the grid's seed, and the command that regenerates them", () => {
    const run = mkdtempSync(join(tmpdir(), "fuzz-rerun-"));
    const unit = join(run, "lg-recall", "REACTIVE_WRITE_IN_OWNED_SCOPE");
    mkdirSync(unit, { recursive: true });
    writeFileSync(join(unit, "legal.json"), JSON.stringify({ seed: "1" }));
    const ids = ["lg-reactive-write-in-owned-scope-handler-store-wrapper-module-helper-m-show-keyed", "lg-reactive-write-in-owned-scope-handler-store-hook-module-helper-m-show-keyed"];
    writeFileSync(join(unit, "programs.jsonl"), [...ids.map((id) => ({ id, code: "REACTIVE_WRITE_IN_OWNED_SCOPE", context: "handler", outcome: "harness", detail: { why: "runtime: Page.enable: no response" } })), { id: "x", code: "REACTIVE_WRITE_IN_OWNED_SCOPE", context: "handler", outcome: "clean" }].map((row) => JSON.stringify(row)).join("\n"));
    const [entry, ...rest] = selectRerun(run, { grids: ["lg-recall", "mx-recall"] });
    expect(rest).toEqual([]);
    expect(entry).toMatchObject({ grid: "lg-recall", code: "REACTIVE_WRITE_IN_OWNED_SCOPE", seed: "1", ids, contexts: ["handler"] });
    expect(rerunCommand(entry, "/m.json", "/out")).toBe("node scripts/fuzz/legal.mjs --grid --recall --seed 1 --codes REACTIVE_WRITE_IN_OWNED_SCOPE --contexts handler --ids /m.json --out /out/lg-recall-REACTIVE_WRITE_IN_OWNED_SCOPE --confirm-with-chrome");
    // The audit's two lg-recall harness programs are programs the grid still generates.
    const generated = new Set(recallLegalShapes("REACTIVE_WRITE_IN_OWNED_SCOPE", "handler").map((shape) => generateLegal({ seed: 1 }, { code: "REACTIVE_WRITE_IN_OWNED_SCOPE", context: "handler" }, { shape }).built.id));
    for (const id of ids) expect(generated.has(id)).toBe(true);
  });
});
