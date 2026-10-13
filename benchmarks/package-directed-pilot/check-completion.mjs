// Observation assertions only; these checks confer no receipt authority.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

export function checkCompletion(document) {
  assert.equal(document.mode, "completion");
  assert.equal(document.results.length, 3);
  const byName = new Map(document.results.map(trial => [trial.trial, trial]));
  assert.equal(byName.size, 3);
  const node = byName.get("event-dispatcher-node-complete");
  const browser = byName.get("event-dispatcher-browser-complete");
  const platform = byName.get("platform-node-complete");
  assert(node && browser && platform);
  for (const trial of byName.values()) {
    assert.equal(trial.result.admitted, true);
    assert.equal(trial.consumers.length, 1);
    const pair = trial.consumers[0];
    for (const part of ["misuse", "correct"]) {
      assert.deepEqual(pair.tsc[part], { status: 0, stdout: "", stderr: "" });
    }
    assert.deepEqual(pair.authored, pair.baseline, "These are replication/refusal controls, not improvements");
    assert(!pair.authored.misuse.some(finding => finding.kind === "violation"));
  }
  assert.deepEqual(node.surface, [{ export: "createEventDispatcher", state: "clean" }]);
  assert.equal(node.withheldOperations.length, 0);
  assert.equal(node.withheldClosures.length, 0);
  assert.deepEqual(node.acceptedSummary.call.closed, ["callbacks", "reads", "creates", "returns"]);
  assert.deepEqual(node.acceptedSummary.call.operations[0].output,
    { kind: "described-callable", reads: [], returns: ["plain"] });
  assert.deepEqual(node.consumers[0].authored.misuse,
    [{ id: "SC1001", rule: "strict-read-untracked", kind: "uncertifiable" }]);
  assert.deepEqual(node.consumers[0].authored.correct, []);
  assert.deepEqual(browser.surface, [{ export: "createEventDispatcher", state: "every-import" }]);
  assert(browser.withheldOperations.length > 0);
  assert.equal(browser.acceptedSummary.call.operations?.length ?? 0, 0);
  for (const part of ["misuse", "correct"]) {
    assert(browser.consumers[0].authored[part].some(finding => finding.id === "SC9005"));
  }
  assert.equal(platform.surface.length, 23);
  assert(platform.surface.every(item => item.state === "value"));
  assert.deepEqual(platform.consumers[0].authored, { misuse: [], correct: [] });
  return { trials: 3, typeChecks: 6, consumerAnalyses: 12, additionalCompletePackages: 0 };
}

if (import.meta.main) console.log(JSON.stringify(checkCompletion(JSON.parse(readFileSync(process.argv[2])))));
