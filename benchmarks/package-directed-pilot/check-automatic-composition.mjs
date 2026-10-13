import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { automaticCompositionCases } from "./automatic-composition-cases.mjs";

export function checkAutomaticComposition(document, probes) {
  assert.equal(document.authority, false);
  assert.equal(document.kind, "automatic-composition-breadth");
  assert.equal(document.authoredProposals, 0);
  for (const key of ["checkerSha256", "producerSha256"]) assert.match(document[key], /^sha256:[a-f0-9]{64}$/);
  const counts = { connectivity: [3, 6], media: [0, 6], mouse: [2, 8], orientation: [1, 2],
    "page-utilities": [2, 4], interaction: [0, 5] };
  assert.equal(document.results.length, automaticCompositionCases.length * 2);
  let typeChecks = 0, consumerAnalyses = 0;
  for (const spec of automaticCompositionCases) {
    const rows = document.results.filter(item => item.package === `@solid-primitives/${spec.name}`);
    assert.deepEqual(rows.map(item => item.host), ["node", "browser"]);
    for (const row of rows) {
      assert.equal(row.version, spec.version);
      assert.deepEqual(row.targets, spec.targets);
      if (spec.name === "drag-drop") {
        assert.equal(row.refused?.stage, "graph-preparation");
        assert.match(row.refused.reason, /\.\/web is not exported by the package/);
        assert.deepEqual(row.consumers, []); continue;
      }
      assert.equal(row.refused, undefined);
      const [nodeClean, total] = counts[spec.name];
      assert.equal(row.surface.length, total);
      assert.equal(row.surface.filter(item => item.state === "clean").length, row.host === "node" ? nodeClean : 0);
      assert.equal(row.complete, false);
      assert.deepEqual(row.consumers.map(item => item.id), ["unowned", "owned", "callback"]);
      for (const consumer of row.consumers) {
        assert.deepEqual(consumer.tsc, { status: 0, stdout: "", stderr: "" }); typeChecks++;
        assert.equal(consumer.generated.accepted, true); consumerAnalyses += 2;
        if (row.host === "node" && nodeClean > 0) {
          assert.equal(consumer.generated.status, "certified");
          assert.deepEqual(consumer.generated.findings, []);
          assert(consumer.baseline.findings.some(item => item.id === "SC9005"));
        } else {
          assert.equal(consumer.generated.status, "uncertifiable");
          assert.deepEqual(consumer.generated.findings, consumer.baseline.findings);
        }
      }
    }
  }
  assert.equal(probes.authority, false);
  assert.equal(probes.kind, "automatic-composition-call-time-observations");
  assert.equal(probes.guardInstalled, "after-module-import");
  assert.deepEqual(probes.results.map(item => item.package), document.results.filter(item => item.host === "node" && !item.refused).map(item => item.package));
  for (const probe of probes.results) {
    assert.equal(probe.status, 0); assert.equal(probe.stderr, "");
    assert.match(probe.artifactSha256, /^sha256:[a-f0-9]{64}$/);
    assert.deepEqual(probe.observation, { calls: 0 });
  }
  return { typeChecks, consumerAnalyses, nodeCleanExports: 8, runtimeExportsPerHost: 31,
    additionalCompleteHostCases: 0, correctedRuntimeProbes: probes.results.length, publishedGraphRefusals: 2 };
}

if (import.meta.main) {
  const evidence = readFileSync(process.argv[2]);
  const probes = JSON.parse(readFileSync(process.argv[3]));
  assert.equal(probes.evidenceSha256, `sha256:${createHash("sha256").update(evidence).digest("hex")}`,
    "Runtime observations belong to different certification evidence");
  console.log(JSON.stringify(checkAutomaticComposition(JSON.parse(evidence), probes)));
}
