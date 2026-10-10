// Checks experimental observations only. This is never receipt authority.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

export function checkExtended(document) {
  assert.equal(document.mode, "extended");
  const expected = new Map([
    ["event-listener-browser-owner-possible", ["pilot-effect-owner", 0, "uncertifiable"]],
    ["event-listener-node-owner-possible", [null, null, null]],
    ["event-listener-none-owner-possible", ["pilot-effect-owner", 0, "uncertifiable"]],
    ["raf-browser-owner-only", ["pilot-cleanup", 1, "violation"]],
    ["raf-node-owner-only", [null, null, null]],
    ["raf-none-owner-only", [null, null, null]],
    ["raf-browser-return-possible", ["pilot-cleanup", 1, null]],
    ["memo-browser-owner-only", ["pilot-cleanup", 1, "violation"]]
  ]);
  assert.equal(document.results.length, expected.size);
  const seen = new Set();
  for (const trial of document.results) {
    assert(expected.has(trial.trial), `Unexpected trial ${trial.trial}`);
    assert(!seen.has(trial.trial), `Duplicate trial ${trial.trial}`);
    seen.add(trial.trial);
    assert.equal(trial.result.admitted, true);
    const [operation, min, kind] = expected.get(trial.trial);
    const operations = trial.acceptedSummary.call?.operations ?? [];
    if (operation) assert.equal(operations.find(item => item.id === operation)?.count?.min, min, trial.trial);
    else {
      assert.equal(operations.length, 0, trial.trial);
      assert(trial.withheldOperations.length > 0, "An unsupported owner claim must have an explicit withholding");
    }
    assert.equal(trial.consumers.length, 1);
    const pair = trial.consumers[0];
    for (const part of ["misuse", "correct"]) {
      assert.equal(pair.tsc[part].status, 0);
      assert.equal(pair.tsc[part].stdout, "");
      assert.equal(pair.tsc[part].stderr, "");
      assert(pair.authored[part].some(item => item.id === "SC9005"), "Incomplete contract must stay uncertifiable");
    }
    assert(!pair.authored.correct.some(item => item.id === "SC4001"), "Correct owner twin must have no owner finding");
    const owner = pair.authored.misuse.filter(item => item.id === "SC4001");
    if (kind) assert.deepEqual(owner.map(item => item.kind), [kind], trial.trial);
    else assert.equal(owner.length, 0, trial.trial);
    if (trial.trial === "raf-browser-owner-only") {
      assert(!pair.baseline.misuse.some(item => item.id === "SC4001"), "RAF must improve over the compiled-tier baseline");
    }
    if (trial.trial === "memo-browser-owner-only") {
      assert.deepEqual(pair.authored, pair.baseline, "Memo is a replication control, not a new finding");
    }
    if (trial.mode === "return-possible") {
      assert(!operations.some(item => item.id === "pilot-return"));
      assert(trial.withheldOperations.some(item => item.reason.includes("unsupported structural member claim")));
      assert.deepEqual(pair.authored, pair.baseline);
    }
  }
  return { trials: seen.size, typeChecks: seen.size * 2, consumerAnalyses: seen.size * 4 };
}

if (import.meta.main) console.log(JSON.stringify(checkExtended(JSON.parse(readFileSync(process.argv[2])))));
