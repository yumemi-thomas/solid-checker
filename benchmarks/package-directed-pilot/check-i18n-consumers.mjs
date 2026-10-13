import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
const document = JSON.parse(readFileSync(process.argv[2]));
assert.equal(document.authority, false); assert.equal(document.kind, "i18n-consumer-twins");
assert.equal(document.results.length, 4);
const targets = ["template", "identityResolveTemplate", "missingKeyAsPath"];
assert.deepEqual(document.results.map(row => row.id), [...targets, "i18n-translator-argument-read"]);
for (const row of document.results) {
  for (const twin of ["misuse", "correct"]) {
    assert.deepEqual(row.twins[twin].tsc, { status: 0, stdout: "", stderr: "" });
    assert.deepEqual(Object.keys(row.twins[twin].hosts), ["none", "browser", "node"]);
    for (const response of Object.values(row.twins[twin].hosts)) {
      if (targets.includes(row.id)) {
        assert.deepEqual(response, twin === "misuse"
          ? { status: "violation", findings: [{ id: "SC1001", rule: "strict-read-untracked", kind: "violation" }] }
          : { status: "certified", findings: [] });
      } else {
        assert.equal(response.status, "uncertifiable");
        assert.equal(response.findings.length, 2);
        assert(response.findings.every(item => item.id === "SC9005" && item.kind === "uncertifiable"));
      }
    }
  }
}
console.log(JSON.stringify({ authority: false, strictTypeChecks: 8, consumerAnalyses: 24,
  correctStrictReadViolations: 9, certifiedCorrectTwins: 9, criterion3Gains: 0 }));
