import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "vitest";
import { feedbackSnapshot, captureTemplate, admitFeedbackCapture, inspectDevelopmentFeedback } from "../scripts/development-feedback.mjs";

const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
function fixture(run) {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "solid-feedback-")));
  const path = join(root, "app.ts"), project = join(root, "tsconfig.json"), runtime = join(root, "runtime.js");
  const source = "export const value = 1;\n";
  writeFileSync(path, source); writeFileSync(runtime, "export const read = () => 1;\n");
  writeFileSync(project, JSON.stringify({ compilerOptions: { strict: true, noEmit: true, types: [], skipLibCheck: false }, files: [path] }));
  try {
    const snapshot = feedbackSnapshot(project), capture = JSON.parse(JSON.stringify(captureTemplate(snapshot)));
    capture.runtimeInputs.push({ path: runtime, sha256: hash(readFileSync(runtime)) });
    capture.events.push({ id: "read-1", kind: "untracked-read", message: "Read while tracking was off", tracking: "untracked",
      runtimeInput: runtime, sourceSha256: hash(source), location: { path, startByte: 13, endByte: 18 } });
    return run({ root, path, project, runtime, snapshot, capture });
  } finally { rmSync(root, { recursive: true, force: true }); }
}
test("capture JSON round trip retains current inputs and an informational read", () => fixture(({ snapshot, capture }) => {
  const result = admitFeedbackCapture(snapshot, capture);
  assert.equal(result.observations[0].severity, "info");
  assert.equal(result.observations[0].reactiveIntent, "open");
  assert.equal(result.observations[0].certification, false);
}));
test("native proven findings retain their own channel and uncertifiable rows", () => fixture(({ project, capture }) => {
  const analysis = { findings: [{ kind: "violation", id: "SC1005" }, { kind: "uncertifiable", id: "SC1001" }] };
  const report = inspectDevelopmentFeedback(project, { capture, analyze: () => ({ status: 0, stdout: JSON.stringify(analysis) }) });
  assert.deepEqual(report.analysis, analysis);
  assert.deepEqual(report.findings, [analysis.findings[0]]);
  assert.deepEqual(report.gaps, [analysis.findings[1]]);
  assert.equal(report.observations.length, 1);
  assert.equal(report.coverage.packageInference, "unavailable");
}));
for (const kind of ["runtime-exception", "assertion-failure"]) {
  test(`admits a supplied ${kind} without claiming native proof`, () => fixture(({ snapshot, capture }) => {
    const read = capture.events[0];
    capture.runtimeInputs = [];
    capture.events = [{ id: "failure-1", kind, message: "Recorded application failure",
      location: read.location, sourceSha256: read.sourceSha256 }];
    const result = admitFeedbackCapture(snapshot, capture);
    assert.equal(result.observations[0].kind, kind);
    assert.equal(result.observations[0].severity, "error");
    assert.equal(result.observations[0].channel, "recorded-runtime");
    assert.equal(result.observations[0].authority, false);
    assert.equal(result.observations[0].certification, false);
  }));
}
for (const [name, edit] of [
  ["source changes", ({ path }) => writeFileSync(path, "export const value = 2;\n")],
  ["runtime changes", ({ runtime }) => writeFileSync(runtime, "export const read = () => 2;\n")],
  ["configuration changes", ({ project }) => writeFileSync(project, "{}")],
  ["forged source digest", ({ capture }) => { capture.events[0].sourceSha256 = hash("other"); }],
  ["duplicate events", ({ capture }) => { capture.events.push(capture.events[0]); }],
  ["unsupported event", ({ capture }) => { capture.events[0].kind = "proven-package-defect"; }],
  ["missing native read input", ({ capture }) => { capture.runtimeInputs = []; }],
  ["authority assertion", ({ capture }) => { capture.authority = true; }],
  ["source span outside file", ({ capture }) => { capture.events[0].location.endByte = 1000; }],
]) test(`refuses ${name}`, () => fixture(state => { edit(state); assert.throws(() => admitFeedbackCapture(state.snapshot, state.capture)); }));
test("inputs changed during native analysis refuse the complete report", () => fixture(({ project, path, capture }) => {
  assert.throws(() => inspectDevelopmentFeedback(project, { capture, analyze: () => {
    writeFileSync(path, "export const value = 2;\n"); return { status: 0, stdout: '{"findings":[]}' };
  } }));
}));
test("repeated observations group without dropping event identities", () => fixture(({ snapshot, capture }) => {
  capture.events.push({ ...capture.events[0], id: "read-2" });
  const result = admitFeedbackCapture(snapshot, capture);
  assert.equal(result.observations.length, 1);
  assert.equal(result.observations[0].occurrences, 2);
  assert.deepEqual(result.observations[0].eventIds, ["read-1", "read-2"]);
}));
test("real TypeScript errors suppress additional captured guidance", () => fixture(({ project, path, runtime }) => {
  writeFileSync(path, 'export const value: number = "wrong";\n');
  const snapshot = feedbackSnapshot(project), capture = captureTemplate(snapshot);
  capture.events.push({ id: "failure-1", kind: "runtime-exception", message: "A recorded failure",
    location: { path, startByte: 13, endByte: 18 }, sourceSha256: hash(readFileSync(path)) });
  assert(snapshot.typingErrors > 0);
  assert.deepEqual(admitFeedbackCapture(snapshot, capture).observations, []);
}));
test("feedback manifest CLI works without the native binary", () => fixture(({ project }) => {
  const result = spawnSync(process.execPath, [new URL("../bin/solid-checker.mjs", import.meta.url).pathname,
    "feedback", "manifest", "--project", project], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(JSON.parse(result.stdout).documentKind, "solid-checker-development-capture");
}));
test("native failure exit status remains visible in the feedback report", () => fixture(({ project }) => {
  const report = inspectDevelopmentFeedback(project, { analyze: () => ({ status: 1, stdout: '{"findings":[]}' }) });
  assert.equal(report.nativeExitCode, 1);
}));
