// Assertions over saved automatic observations. This does not grant authority.
import assert from "node:assert/strict";
import { readFileSync, existsSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
const [beforeFile, afterFile, noneFile, output] = process.argv.slice(2);
assert(beforeFile && afterFile && noneFile && output && !existsSync(output));
const read = path => JSON.parse(readFileSync(path));
const [before, after, none] = [beforeFile, afterFile, noneFile].map(read);
for (const document of [before, after, none]) {
  assert.equal(document.authority, false); assert.equal(document.authoredProposals, 0);
  assert.equal(document.kind, "automatic-composition-breadth");
  assert.equal(document.producerSha256, before.producerSha256);
}
assert.notEqual(before.checkerSha256, after.checkerSha256);
assert.equal(after.checkerSha256, none.checkerSha256);
assert.equal(before.results.length, 2); assert.equal(after.results.length, 2); assert.equal(none.results.length, 1);
const targets = ["identityResolveTemplate", "missingKeyAsPath", "template"];
const result = { authority: false, kind: "parameter-passthrough-comparison", results: [], strictTypeChecks: 0, consumerAnalyses: 0 };
for (const row of [...after.results, ...none.results]) {
  assert.equal(row.package, "@solid-primitives/i18n"); assert.equal(row.version, "3.0.0-next.4");
  assert.equal(row.refused, undefined); assert.equal(row.complete, false);
  assert.equal(row.surface.length, 12);
  assert.deepEqual(row.surface.filter(item => item.state === "clean").map(item => item.export), targets);
  assert(!row.withheldClosures.some(item => item.node.package === row.package && targets.includes(item.export)));
  const old = before.results.find(item => item.host === row.host);
  if (old) {
    assert.equal(old.integrity, row.integrity);
    assert(!old.surface.some(item => item.state === "clean"));
    assert.deepEqual(old.surface.filter(item => !targets.includes(item.export)), row.surface.filter(item => !targets.includes(item.export)));
    const generated = (file, host) => read(join(dirname(file), `i18n-${host}`, "generated-inputs.json"))
      .map(({ package: name, sha256 }) => ({ package: name, sha256 }));
    assert.deepEqual(generated(beforeFile, row.host), generated(afterFile, row.host), "Proposals must remain byte-identical");
    // Main-document digests change with the three new closures, and diagnostic
    // paths name a fresh private producer project. Compare the remaining exact
    // claim addresses and reasons after normalizing only that scratch component.
    const remaining = items => items.filter(item => item.node.package === row.package && !targets.includes(item.export))
      .map(({ node, reason, ...item }) => ({ ...item, package: node.package, version: node.version,
        reason: reason.replace(/solid-checker-typefacts-project-\d+-\d+/g, "solid-checker-typefacts-project-SCRATCH") }));
    assert.deepEqual(remaining(old.withheldClosures), remaining(row.withheldClosures));
  }
  for (const consumer of row.consumers) {
    assert.deepEqual(consumer.tsc, { status: 0, stdout: "", stderr: "" });
    assert.deepEqual(consumer.generated, { status: "certified", accepted: true, findings: [] });
    assert.equal(consumer.baseline.status, "uncertifiable");
    assert.equal(consumer.baseline.findings.length, 3);
    assert(consumer.baseline.findings.every(item => item.id === "SC9005"));
    result.strictTypeChecks++; result.consumerAnalyses += 2;
  }
  result.results.push({ host: row.host, clean: 3, total: 12, complete: false, changed: targets });
}
writeFileSync(output, JSON.stringify(result, null, 2) + "\n");
console.log(JSON.stringify(result));
