// Independent input/result audit. Does not import the feedback detector.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const [stockArg, candidateArg, outputArg] = process.argv.slice(2);
assert(stockArg && candidateArg && outputArg, "Supply stock directory, candidate directory and fresh audit path");
const stock = resolve(stockArg), candidate = resolve(candidateArg), output = resolve(outputArg);
assert(!existsSync(output));
const read = path => JSON.parse(readFileSync(path, "utf8"));
const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const inputs = [];
function evidence(path) { inputs.push({ path, sha256: hash(readFileSync(path)) }); return read(path); }
const reports = [stock, candidate].map(directory => {
  const report = evidence(join(directory, "results.json"));
  assert.equal(report.authority, false); assert.equal(report.certification, false); assert(report.finishedAt && report.preservedInputs);
  for (const [index, pin] of report.implementation.entries()) {
    const path = index === 0 ? join(directory, "evaluation-runner.mjs") : pin.path;
    assert.equal(hash(readFileSync(path)), pin.sha256, `Implementation input changed: ${path}`);
  }
  for (const row of report.results) {
    assert(row.preservedInputs); assert.equal(row.initialGitStatus, row.finalGitStatus);
    for (const pin of row.inputs) assert.equal(hash(readFileSync(join(row.root, pin.path))), pin.sha256);
    for (const pin of [...row.runtimePackages, row.typescript]) assert.equal(hash(readFileSync(pin.metadata)), pin.sha256);
    const run = row.executions.feedback;
    if (run.error) {
      assert.equal(row.id, "finds-team"); assert.equal(run.error.code, "ETIMEDOUT");
      assert.equal(readFileSync(join(directory, row.id + ".feedback.stdout.log"), "utf8"), "");
    } else {
      const feedback = evidence(join(directory, row.id + ".feedback.json"));
      assert.deepEqual(feedback, read(join(directory, row.id + ".feedback.stdout.log")));
      assert.equal(feedback.typingErrorCount, 0);
      assert(feedback.findings.every(row => row.kind === "violation"));
      assert(feedback.gaps.every(row => row.kind === "uncertifiable"));
    }
  }
  return report;
});
for (const row of reports[0].results) assert.equal(row.executions.typing.status, 0, "Installed published typing check failed");
const findings = [];
for (const id of ["helge-dev", "oscartbeaumont-website"]) {
  const original = read(join(stock, id + ".feedback.json")), comparison = read(join(candidate, id + ".feedback.json"));
  assert.deepEqual(original.findings, comparison.findings, "Compiler profiles changed findings");
  assert.deepEqual(original.gaps, comparison.gaps, "Compiler profiles changed proof obligations");
  for (const finding of original.findings) {
    // These are reviewed dispositions of this exact population, not rules for
    // classifying arbitrary findings or a replacement semantic implementation.
    const disposition = finding.id === "SC2003" ? "incorrect-nested-write-claim" :
      finding.id === "SC1001" ? "incorrect-deferred-event-read-claim" :
      finding.id === "SC8015" ? "preference" : "unvalidated-loading-context";
    findings.push({ application: id, nativeKind: finding.kind, nativeRule: finding.rule, id: finding.id,
      location: finding.primaryLocation, disposition });
  }
}
assert.equal(findings.filter(row => row.disposition === "incorrect-nested-write-claim").length, 14);
assert.equal(findings.filter(row => row.disposition === "incorrect-deferred-event-read-claim").length, 1);
assert.equal(findings.filter(row => row.disposition === "preference").length, 2);
assert.equal(findings.filter(row => row.disposition === "unvalidated-loading-context").length, 2);
const boundary = evidence(join(stock, "boundary-review.json"));
assert(boundary.preservedSource && boundary.aliasContrast);
assert(boundary.results.find(row => row.boundary === "nested-props-mutable-wrapper")?.writesRetained);
for (const pin of boundary.sources) assert.equal(hash(readFileSync(pin.path)), pin.sha256);
const existing = evidence(join(stock, "existing-test-audit.json"));
assert.deepEqual(existing.summary, { passedAssertions: 24, files: 8, failedAssertions: 0, skippedAssertions: 0 });
const browserTests = evidence(join(stock, "helge-existing-tests/results.json"));
const browserRaw = evidence(join(stock, "helge-existing-tests/playwright.json"));
assert.equal(browserTests.status, 0); assert(browserTests.preservedSource);
assert.deepEqual(browserTests.stats, browserRaw.stats);
assert.equal(browserTests.stats.expected, 4);
for (const key of ["unexpected", "skipped", "flaky"]) assert.equal(browserTests.stats[key], 0);
for (const pin of browserTests.inputs) {
  assert.equal(hash(readFileSync(join(browserTests.source, pin.path))), pin.sha256);
  assert.equal(hash(readFileSync(join(browserTests.clone, pin.path))), pin.sha256);
}
const profileRoot = join(stock, "client-profile");
const live = evidence(join(profileRoot, "stdout.log"));
const baseCollector = readFileSync("packages/cli/scripts/feedback-browser.mjs", "utf8");
const adaptedCollector = readFileSync(join(profileRoot, "scripts/feedback-browser.mjs"), "utf8");
assert.equal(adaptedCollector, baseCollector.replace("root, cacheDir: cache, publicDir: false", "root, cacheDir: cache, publicDir: \"static\"")
  .replace('resolve: { dedupe: ["solid-js", "@solidjs/signals", "@solidjs/web"] }',
    'resolve: { alias: { "solid-js/web": "@solidjs/web" }, dedupe: ["solid-js", "@solidjs/signals", "@solidjs/web"] }'));
for (const name of ["development-feedback", "feedback-native-hook", "feedback-read-runtime", "feedback-read-selector", "feedback-source-hook", "feedback-assertion-selector"]) {
  assert.equal(hash(readFileSync(join(profileRoot, `scripts/${name}.mjs`))), hash(readFileSync(`packages/cli/scripts/${name}.mjs`)));
}
assert.equal(live.typingErrorCount, 0); assert.equal(live.assertions.length, 4);
assert(live.assertions.every(row => row.passed)); assert.deepEqual(live.assertionFailures, []);
assert.deepEqual(live.execution.pageErrors, []); assert.deepEqual(live.execution.blockedRequests, []);
assert.deepEqual(live.automatic.notes, []);
const coverage = live.coverage.nativeReadCollection;
assert.equal(hash(readFileSync(coverage.nativeReader.path)), coverage.nativeReader.sha256);
for (const model of live.analysis.feedbackFacts) assert.equal(hash(readFileSync(model.path)), model.sourceSha256);
assert.equal(live.execution.stats.retained, 6); assert.equal(live.execution.unmapped.length, 6);
assert.equal(live.execution.stats.dropped, 0); assert.equal(live.automatic.complete, false);
const result = { authority: false, certification: false, inputs, reviewedFindings: findings,
  summary: { configurations: 3, installedTypingChecksPassed: 3, completedNativeConfigurationsPerProfile: 2,
    timedOutConfigurationsPerProfile: 1, reviewedNativeFindings: 19, incorrectClaims: 15, preferences: 2,
    unvalidatedLoadingContexts: 2, confirmedApplicationDefects: 0, existingUnitTestsPassed: 24,
    existingBrowserTestsPassed: 4, adaptedLiveAssertionsPassed: 4, adaptedLiveAutomaticNotes: 0,
    adaptedLiveUnmappedReads: 6 },
  limits: ["Convenience selection of three retained projects, not a representative accuracy population.",
    "SSR helper execution disproves dropped writes, not browser DOM/lifecycle correctness.",
    "The successful live collector has an explicitly adapted development configuration.",
    "A quiet detector with six unmapped records does not establish complete or precise package coverage.",
    "Two loading warnings remain unvalidated; no repaired application or positive real-application defect is claimed."] };
writeFileSync(output, JSON.stringify(result, null, 2) + "\n");
console.log(JSON.stringify(result.summary));
