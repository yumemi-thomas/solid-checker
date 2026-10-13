// Compare saved observations; never repeat certification or confer authority.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, existsSync, writeFileSync } from "node:fs";
const [beforeFile, afterFile, output] = process.argv.slice(2);
assert(beforeFile && afterFile && output && !existsSync(output));
const files = [beforeFile, afterFile].map(path => readFileSync(path));
const [before, after] = files.map(bytes => JSON.parse(bytes));
for (const document of [before, after]) {
  assert.equal(document.authority, false); assert.equal(document.authoredProposals, 0);
  assert.equal(document.kind, "automatic-composition-breadth");
}
assert.equal(before.producerSha256, after.producerSha256);
assert.notEqual(before.checkerSha256, after.checkerSha256);
assert.equal(before.results.length, after.results.length);
const result = { authority: false, kind: "composition-replay-comparison",
  inputs: files.map(bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`),
  rootRefusalChanges: [], strictTypeChecks: 0, consumerAnalyses: 0, cleanExports: { node: 0, browser: 0 }, exportsPerHost: {} };
for (const row of after.results) {
  const old = before.results.find(item => item.package === row.package && item.host === row.host);
  assert(old); assert.equal(old.version, row.version); assert.equal(old.integrity, row.integrity);
  assert.deepEqual(row.surface, old.surface); assert.deepEqual(row.complete, old.complete);
  assert.deepEqual(row.refused, old.refused);
  if (row.refused) continue;
  assert.deepEqual(row.consumers, old.consumers);
  for (const consumer of row.consumers) {
    assert.deepEqual(consumer.tsc, { status: 0, stdout: "", stderr: "" });
    assert.equal(consumer.generated.accepted, true);
    result.strictTypeChecks++; result.consumerAnalyses += 2;
  }
  result.cleanExports[row.host] += row.surface.filter(item => item.state === "clean").length;
  result.exportsPerHost[row.host] = (result.exportsPerHost[row.host] ?? 0) + row.surface.length;
  if (row.host === "node") assert.deepEqual(row.runtime, { calls: 0 });
  for (const refusal of row.withheldClosures.filter(item => item.node.package === row.package)) {
    const previous = old.withheldClosures.find(item => item.node.package === row.package && item.export === refusal.export && item.domain === refusal.domain);
    if (previous?.reason === "no recipe in corpus" && refusal.reason !== previous.reason)
      result.rootRefusalChanges.push({ package: row.package, host: row.host, export: refusal.export,
        domain: refusal.domain, before: previous.reason, after: refusal.reason });
  }
}
assert.equal(result.strictTypeChecks, 36); assert.equal(result.consumerAnalyses, 72);
assert.deepEqual(result.cleanExports, { node: 8, browser: 0 });
writeFileSync(output, JSON.stringify(result, null, 2) + "\n");
console.log(JSON.stringify(result));
