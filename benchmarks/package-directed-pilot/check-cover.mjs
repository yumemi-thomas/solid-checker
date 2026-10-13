// Experimental observations, not proof or receipt authority.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

export function checkCover(document) {
  assert.equal(document.mode, "cover");
  const expected = new Map([
    ["event-listener-browser-owner-guaranteed", ["pilot-effect-owner", "violation"]],
    ["event-listener-node-owner-guaranteed", [null, null]],
    ["event-listener-none-owner-guaranteed", [null, null]],
    ["raf-browser-owner-only", ["pilot-cleanup", "violation"]],
    ["memo-browser-owner-only", ["pilot-cleanup", "violation"]]
  ]);
  assert.equal(document.results.length, expected.size);
  const seen = new Set();
  for (const trial of document.results) {
    assert(expected.has(trial.trial) && !seen.has(trial.trial));
    seen.add(trial.trial);
    assert.equal(trial.result.admitted, true);
    const [operation, kind] = expected.get(trial.trial);
    const operations = trial.acceptedSummary.call.operations ?? [];
    if (operation) assert.equal(operations.find(item => item.id === operation)?.count?.min, 1);
    else {
      assert.equal(operations.length, 0);
      assert(trial.withheldOperations.length > 0);
    }
    assert.equal(trial.consumers.length, 1);
    const pair = trial.consumers[0];
    for (const part of ["misuse", "correct"]) {
      assert.deepEqual(pair.tsc[part], { status: 0, stdout: "", stderr: "" });
      assert(pair.authored[part].some(item => item.id === "SC9005"));
    }
    assert.deepEqual(pair.authored.misuse.filter(item => item.id === "SC4001").map(item => item.kind), kind ? [kind] : []);
    assert(!pair.authored.correct.some(item => item.id === "SC4001"));
    if (trial.trial.startsWith("event-listener-browser")) {
      assert.deepEqual(pair.baseline.misuse.filter(item => item.id === "SC4001").map(item => item.kind), ["uncertifiable"]);
    }
    if (trial.trial.startsWith("memo-")) assert.deepEqual(pair.authored, pair.baseline);
  }
  return { trials: 5, typeChecks: 10, consumerAnalyses: 20, additionalCompletePackages: 0 };
}

export function checkBounds(document) {
  assert.equal(document.mode, "bounds");
  const expected = new Map([["lifecycle", 1], ["permission", 2], ["sensors", 2], ["timer", 4], ["workers", 1]]);
  assert.equal(document.results.length, 5);
  let pairs = 0, retained = 0, attempted = 0;
  const seen = new Set();
  for (const trial of document.results) {
    const name = trial.package.replace("@solid-primitives/", "");
    assert(expected.has(name) && !seen.has(name));
    seen.add(name);
    assert.equal(trial.result.admitted, true);
    const upgraded = trial.ownerBounds.filter(item => item.originalMin === 0);
    assert.equal(upgraded.length, expected.get(name));
    attempted += upgraded.length;
    assert(upgraded.every(item => item.acceptedMin === null), "These path-dependent claims must be withheld");
    const guaranteed = trial.ownerBounds.filter(item => item.originalMin === 1);
    assert(guaranteed.every(item => item.acceptedMin === 1), "Existing guaranteed claims must survive");
    retained += guaranteed.length;
    for (const pair of trial.consumers) {
      pairs++;
      for (const part of ["misuse", "correct"]) {
        assert.deepEqual(pair.tsc[part], { status: 0, stdout: "", stderr: "" });
        assert(pair.authored[part].some(item => item.id === "SC9005"));
        assert(!pair.authored[part].some(item => item.kind === "violation"));
      }
    }
  }
  assert.equal(pairs, 4);
  assert.equal(retained, 6);
  return { trials: 5, proposedStrongerBounds: attempted, acceptedStrongerBounds: 0, retainedGuaranteedBounds: retained,
    typeChecks: pairs * 2, consumerAnalyses: pairs * 4 };
}

if (import.meta.main) {
  const document = JSON.parse(readFileSync(process.argv[2]));
  console.log(JSON.stringify(document.mode === "cover" ? checkCover(document) : checkBounds(document)));
}
