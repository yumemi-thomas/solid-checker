// Saved baseline assertions; no proof or receipt authority.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

export function checkComposition(document) {
  assert.equal(document.authority, false);
  assert.equal(document.kind, "composition-baseline");
  assert.equal(document.package, "@solid-primitives/timer");
  assert.equal(document.version, "1.4.5-next.1");
  assert.match(document.checkerSha256, /^sha256:[a-f0-9]{64}$/);
  assert.match(document.producerSha256, /^sha256:[a-f0-9]{64}$/);
  assert.deepEqual(document.results.map(item => item.host), ["node", "browser"]);
  for (const observed of document.results) {
    assert.deepEqual(observed.surface.map(item => item.export).sort(),
      ["createIntervalCounter", "createPolled", "createTimeoutLoop", "createTimer", "makeTimer"].sort());
    assert.equal(observed.complete, false);
    assert.deepEqual(observed.consumers.map(item => item.id),
      ["whole-surface", "untracked-callback", "intentional-snapshot", "captured-callable"]);
    for (const consumer of observed.consumers) {
      assert.deepEqual(consumer.tsc, { status: 0, stdout: "", stderr: "" });
      assert.deepEqual(consumer.authored, consumer.baseline);
    }
  }
  const node = document.results[0];
  assert.equal(node.surface.filter(item => item.state === "clean").length, 4);
  assert.equal(node.surface.find(item => item.export === "createPolled").state, "some-uses");
  for (const id of ["whole-surface", "intentional-snapshot", "captured-callable"]) {
    assert.deepEqual(node.consumers.find(item => item.id === id).authored,
      [{ id: "SC9005", rule: "package-contract-incomplete", kind: "uncertifiable" }]);
  }
  assert(node.consumers.find(item => item.id === "untracked-callback").authored.some(item => item.id === "SC1001" && item.kind === "violation"));
  assert.equal(document.results[1].surface.filter(item => item.state === "clean").length, 0);
  assert.deepEqual(document.runtime, { calls: 1, same: true, callable: true, value: 42 });
  return { typeChecks: 8, consumerAnalyses: 16, additionalCompletePackages: 0, nodeCleanExports: 4, browserCleanExports: 0 };
}

if (import.meta.main) console.log(JSON.stringify(checkComposition(JSON.parse(readFileSync(process.argv[2])))));
