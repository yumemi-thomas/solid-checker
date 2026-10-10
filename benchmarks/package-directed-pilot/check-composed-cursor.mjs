import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

export function checkComposedCursor(document) {
  assert.equal(document.authority, false);
  assert.equal(document.kind, "composed-cursor");
  assert.equal(document.package, "@solid-primitives/cursor");
  assert.equal(document.version, "1.0.0-next.2");
  for (const key of ["checkerSha256", "producerSha256"]) assert.match(document[key], /^sha256:[a-f0-9]{64}$/);
  const negative = document.openDependency || document.withheldDependency;
  assert.deepEqual(document.results.map(item => item.host), negative ? ["node"] : ["node", "browser"]);
  for (const result of document.results) {
    assert.deepEqual(result.surface.map(item => item.export).sort(),
      ["createBodyCursor", "createDragCursor", "createElementCursor", "cursorRef", "makeBodyCursor", "makeElementCursor"].sort());
    assert.deepEqual(result.consumers.map(item => item.id), ["whole-surface", "owned-surface", "callback-control"]);
    for (const consumer of result.consumers) assert.deepEqual(consumer.tsc, { status: 0, stdout: "", stderr: "" });
  }
  const node = document.results[0];
  assert.equal(node.complete, !negative);
  assert.equal(node.surface.filter(item => item.state === "clean").length, negative ? 4 : 6);
  for (const consumer of node.consumers) {
    const count = consumer.id === "callback-control" ? 0 : negative ? 2 : 0;
    assert.deepEqual(consumer.authored, Array.from({ length: count }, () =>
      ({ id: "SC9005", rule: "package-contract-incomplete", kind: "uncertifiable" })));
    assert.equal(consumer.authoredStatus, count ? "uncertifiable" : "certified");
    assert.equal(consumer.authoredAccepted, true);
  }
  if (document.withheldDependency) assert(node.withheldClosures.some(item =>
    item.node?.package === "@solid-primitives/utils" && item.export === "noop" && item.domain === "returns"));
  if (!negative) {
    const browser = document.results[1];
    assert.equal(browser.complete, false);
    assert.equal(browser.surface.filter(item => item.state === "clean").length, 0);
    for (const consumer of browser.consumers) assert.deepEqual(consumer.authored, consumer.baseline);
    const ownerCount = consumer => consumer.authored.filter(item => item.id === "SC4001" && item.kind === "violation").length;
    assert.equal(ownerCount(browser.consumers[0]), 3);
    assert.equal(ownerCount(browser.consumers[1]), 0);
    assert.equal(ownerCount(browser.consumers[2]), 1);
  }
  assert.deepEqual(document.runtime, { calls: 0, returnedUndefined: true });
  return { typeChecks: document.results.length * 3, consumerAnalyses: document.results.length * 6,
    nodeCleanExports: negative ? 4 : 6, additionalCompleteHostCases: negative ? 0 : 1 };
}

if (import.meta.main) console.log(JSON.stringify(checkComposedCursor(JSON.parse(readFileSync(process.argv[2])))));
