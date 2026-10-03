import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "vitest";
import { feedbackSnapshot, captureTemplate, admitFeedbackCapture, inspectDevelopmentFeedback,
  DEFAULT_NATIVE_TIMEOUT_MS, NATIVE_TIMEOUT_ENV, describeNativeBinary, expandSolutionProject, nativeTimeoutMs,
  parseNativeStderr, validateFeedbackInputs, SolutionProjectError } from "../scripts/development-feedback.mjs";

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

// ---------------------------------------------------------------------------
// Native child handling, project references, solution-style projects.
// ---------------------------------------------------------------------------
const cli = new URL("../bin/solid-checker.mjs", import.meta.url).pathname;
const stage = (name, ns = 1000) => JSON.stringify({ reactiveIrStage: name, elapsedNs: ns });
function workspace(run) {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "solid-feedback-native-")));
  try { return run(root); } finally { rmSync(root, { recursive: true, force: true }); }
}
function leaf(root, name, extra = {}) {
  const dir = join(root, name);
  mkdirSync(join(dir, "src"), { recursive: true });
  writeFileSync(join(dir, "src", "index.ts"), `export const ${name.replace(/\W/g, "_")} = 1;\n`);
  writeFileSync(join(dir, "tsconfig.json"), JSON.stringify({ compilerOptions: { strict: true, types: [], composite: true, rootDir: "src", outDir: "dist",
    ...extra.compilerOptions }, include: ["src"], ...(extra.references ? { references: extra.references } : {}) }));
  return join(dir, "tsconfig.json");
}
function fakeNative(root, body, directory = root) {
  mkdirSync(directory, { recursive: true });
  const path = join(directory, "solid-checker-rust");
  writeFileSync(path, `#!/bin/sh\n${body}\n`); chmodSync(path, 0o755);
  return path;
}
function feedbackCli(args, { binary, env = {} } = {}) {
  const run = spawnSync(process.execPath, [cli, "feedback", ...args, ...(binary ? ["--native-bin", binary] : [])],
    { encoding: "utf8", maxBuffer: 1 << 28, env: { ...process.env, SOLID_CHECKER_TIMINGS: "", ...env } });
  let json = null;
  try { json = JSON.parse(run.stdout); } catch { /* stdout is not a document */ }
  return { ...run, json };
}

test("native stderr separates stage lines, the summary and free text", () => {
  const parsed = parseNativeStderr(`HTML is malformed\n${stage("source-discovery")}\n${stage("static-prepass", 9)}\n{"totalNs":5,"irNs":2}\n{ not json }\n`);
  assert.deepEqual(parsed.stages.map(item => item.stage), ["source-discovery", "static-prepass"]);
  assert.deepEqual(parsed.summary, { totalNs: 5, irNs: 2 });
  assert.equal(parsed.text, "HTML is malformed\n{ not json }");
  assert.deepEqual(parseNativeStderr(undefined).stages, []);
});
test("a very long native stderr is truncated from the front", () => {
  const parsed = parseNativeStderr("x".repeat(40000) + "TAIL");
  assert(parsed.truncated && parsed.text.endsWith("TAIL") && parsed.text.length < 17000 && parsed.bytes === 40004);
});
test("native timeout is configurable, defaults to ten minutes, 0 disables, nonsense is refused", () => {
  assert.equal(DEFAULT_NATIVE_TIMEOUT_MS, 600000);
  assert.equal(nativeTimeoutMs("30"), 30000);
  assert.equal(nativeTimeoutMs("0.5"), 500);
  assert.equal(nativeTimeoutMs("0"), 0);
  assert.equal(nativeTimeoutMs(""), DEFAULT_NATIVE_TIMEOUT_MS);
  for (const bad of ["-1", "soon", "NaN"]) assert.throws(() => nativeTimeoutMs(bad), /number of seconds/);
  assert.equal(NATIVE_TIMEOUT_ENV, "SOLID_CHECKER_FEEDBACK_NATIVE_TIMEOUT");
});
test("binary kind is read from the cargo profile directory", () => {
  assert.equal(describeNativeBinary("/w/rust/target/debug/solid-checker-rust").kind, "debug");
  assert.equal(describeNativeBinary("/w/rust/target/release/solid-checker-rust").kind, "release");
  assert.equal(describeNativeBinary("/w/bin/solid-checker-rust").kind, "unknown");
});
test("a timeout returns a structured partial result naming the phase reached", () => workspace(root => {
  const project = leaf(root, "app");
  const binary = fakeNative(root, `echo 'compiler warning text' >&2\necho '${stage("source-discovery")}' >&2\necho '${stage("static-prepass")}' >&2\nexec sleep 30`);
  const started = Date.now();
  const run = feedbackCli(["--project", project, "--native-timeout", "1"], { binary });
  assert(Date.now() - started < 15000, "the child was not killed at the limit");
  assert.equal(run.status, 2);
  assert.match(run.stderr, /exceeded its 1\.0 s limit/);
  const failure = run.json.failure;
  assert.equal(run.json.status, "native-failed");
  assert.equal(failure.kind, "timeout");
  assert.equal(failure.timeoutMs, 1000);
  assert.equal(failure.phase.lastCompletedStage, "static-prepass");
  assert.equal(failure.phase.completedStages, 2);
  assert.match(failure.stderr.text, /compiler warning text/);
  assert.equal(failure.typingErrorCount, 0);
  assert.equal(failure.binary.path, binary);
}));
test("a timeout before any stage line says no stage was reached", () => workspace(root => {
  const binary = fakeNative(root, "exec sleep 30");
  const run = feedbackCli(["--project", leaf(root, "app"), "--native-timeout", "0.5"], { binary });
  assert.equal(run.json.failure.phase.lastCompletedStage, null);
  assert.match(run.json.failure.phase.description, /before the first reactive-IR stage/);
}));
test("the timeout can also come from the environment", () => workspace(root => {
  const binary = fakeNative(root, "exec sleep 30");
  const run = feedbackCli(["--project", leaf(root, "app")], { binary, env: { [NATIVE_TIMEOUT_ENV]: "0.5" } });
  assert.equal(run.json.failure.kind, "timeout");
  assert.equal(run.json.failure.timeoutMs, 500);
}));
test("a native crash keeps its exit status and stderr", () => workspace(root => {
  const binary = fakeNative(root, "echo 'panicked at boom' >&2\nexit 101");
  const run = feedbackCli(["--project", leaf(root, "app")], { binary });
  assert.equal(run.status, 2);
  assert.equal(run.json.failure.kind, "exit-status");
  assert.equal(run.json.failure.exitStatus, 101);
  assert.match(run.stderr, /panicked at boom/);
}));
test("malformed native output is a structured failure", () => workspace(root => {
  const binary = fakeNative(root, "echo 'not json'");
  const run = feedbackCli(["--project", leaf(root, "app")], { binary });
  assert.equal(run.json.failure.kind, "malformed-output");
  assert.equal(run.json.failure.stdoutHead.trim(), "not json");
}));
test("native stderr is surfaced and timings only appear when requested", () => workspace(root => {
  const binary = fakeNative(root, `echo 'compiler note' >&2\necho '${stage("source-discovery", 7)}' >&2\necho '{"totalNs":99}' >&2\necho '{"findings":[]}'`);
  const project = leaf(root, "app");
  const plain = feedbackCli(["--project", project], { binary });
  assert.equal(plain.status, 0, plain.stderr);
  assert.equal(plain.json.native.stderr.text, "compiler note");
  assert.equal(plain.json.native.timings, undefined);
  assert.equal(plain.json.native.binary.kind, "unknown");
  const timed = feedbackCli(["--project", project], { binary, env: { SOLID_CHECKER_TIMINGS: "1" } });
  assert.deepEqual(timed.json.native.timings, { stages: [{ stage: "source-discovery", elapsedNs: 7 }], summary: { totalNs: 99 } });
  assert.equal(timed.json.native.stderr.text, "compiler note");
}));
test("native output larger than spawnSync's default 1 MiB buffer is accepted", () => workspace(root => {
  const big = join(root, "big.json");
  writeFileSync(big, JSON.stringify({ findings: [{ kind: "violation", id: "SC1", message: "x".repeat(3 << 20) }] }));
  const binary = fakeNative(root, `cat '${big}'`);
  const run = feedbackCli(["--project", leaf(root, "app")], { binary });
  assert.equal(run.status, 0, run.stderr);
  assert.equal(run.json.findings.length, 1);
}));
test("a missing explicit native binary is refused, not replaced by a repository build", () => workspace(root => {
  const run = feedbackCli(["--project", leaf(root, "app"), "--native-bin", join(root, "nope")]);
  assert.equal(run.status, 2);
  assert.match(run.stderr, /SOLID_CHECKER_NATIVE_BIN does not exist/);
}));
test("a debug build on a large project warns; a small project and a release build do not", () => workspace(root => {
  const files = [];
  mkdirSync(join(root, "big"));
  for (let index = 0; index < 55; index++) { files.push(`f${index}.ts`); writeFileSync(join(root, "big", files.at(-1)), `export const v${index} = ${index};\n`); }
  writeFileSync(join(root, "big", "tsconfig.json"), JSON.stringify({ compilerOptions: { strict: true, noEmit: true, types: [] }, files }));
  const body = "echo '{\"findings\":[]}'";
  const debug = fakeNative(root, body, join(root, "target", "debug")), release = fakeNative(root, body, join(root, "target", "release"));
  const big = join(root, "big", "tsconfig.json");
  const warned = feedbackCli(["--project", big], { binary: debug });
  assert.match(warned.json.warnings[0], /debug build.*55-file project/);
  assert.match(warned.stderr, /warning: The native checker is a debug build/);
  assert.equal(feedbackCli(["--project", big], { binary: release }).json.warnings, undefined);
  assert.equal(feedbackCli(["--project", leaf(root, "small")], { binary: debug }).json.warnings, undefined);
}));

test("a project that lists references is checked against referenced sources and pins the referenced configs", () => workspace(root => {
  const lib = leaf(root, "lib");
  const app = leaf(root, "app", { references: [{ path: "../lib" }] });
  writeFileSync(join(root, "app", "src", "use.ts"), 'import { lib } from "../../lib/src/index";\nexport const n: number = lib;\n');
  // rootDir would reject the import without the editor-style source redirect; an unbuilt reference alone is TS6305.
  const snapshot = feedbackSnapshot(app);
  assert.equal(snapshot.typingErrors, 0);
  assert.deepEqual(snapshot.manifest.projectReferences, [lib]);
  assert(snapshot.manifest.inputs.some(input => input.kind === "readFile" && input.path === lib));
  writeFileSync(lib, JSON.stringify({ include: ["src"], compilerOptions: { composite: true, strict: true, rootDir: "src", outDir: "dist", types: [], noImplicitAny: false } }));
  assert.throws(() => validateFeedbackInputs(snapshot.manifest), /Feedback input changed/);
}));
test("a referenced project's own type errors belong to that project's check, not the referencing one", () => workspace(root => {
  const lib = leaf(root, "lib");
  writeFileSync(join(root, "lib", "src", "index.ts"), 'export const lib: number = "wrong";\n');
  const app = leaf(root, "app", { references: [{ path: "../lib" }] });
  writeFileSync(join(root, "app", "src", "use.ts"), 'import { lib } from "../../lib/src/index";\nexport const n = lib;\n');
  assert.equal(feedbackSnapshot(app).typingErrors, 0);
  assert(feedbackSnapshot(lib).typingErrors > 0);
}));
test("a solution-style tsconfig is refused by the snapshot with its leaf projects", () => workspace(root => {
  const a = leaf(root, "a"), b = leaf(root, "b");
  writeFileSync(join(root, "tsconfig.json"), JSON.stringify({ files: [], references: [{ path: "./a" }, { path: "./b/tsconfig.json" }] }));
  assert.throws(() => feedbackSnapshot(join(root, "tsconfig.json")), error => error instanceof SolutionProjectError
    && JSON.stringify(error.leaves) === JSON.stringify([a, b]) && error.message.includes(a));
}));
test("solution expansion recurses, de-duplicates, tolerates cycles and fails closed on a missing reference", () => workspace(root => {
  const a = leaf(root, "a"), b = leaf(root, "b");
  mkdirSync(join(root, "inner"));
  writeFileSync(join(root, "inner", "tsconfig.json"), JSON.stringify({ files: [], references: [{ path: "../a" }, { path: "../tsconfig.json" }] }));
  writeFileSync(join(root, "tsconfig.json"), JSON.stringify({ files: [], references: [{ path: "./inner" }, { path: "./a" }, { path: "./b" }] }));
  assert.deepEqual(expandSolutionProject(join(root, "tsconfig.json")), [a, b]);
  assert.deepEqual(expandSolutionProject(a), [a]);
  writeFileSync(join(root, "tsconfig.json"), JSON.stringify({ files: [], references: [{ path: "./missing" }] }));
  assert.throws(() => expandSolutionProject(join(root, "tsconfig.json")), /Referenced project not found/);
}));
test("feedback on a solution-style tsconfig analyses each referenced project and aggregates", () => workspace(root => {
  const a = leaf(root, "a"), b = leaf(root, "b");
  writeFileSync(join(root, "tsconfig.json"), JSON.stringify({ files: [], references: [{ path: "./a" }, { path: "./b" }] }));
  const binary = fakeNative(root, `echo '{"findings":[{"kind":"violation","id":"V","project":"'"$2"'"},{"kind":"uncertifiable","id":"U"}]}'`);
  const run = feedbackCli(["--project", join(root, "tsconfig.json")], { binary });
  assert.equal(run.status, 0, run.stderr);
  assert.deepEqual(run.json.solution.referencedProjects, [a, b]);
  assert.deepEqual(run.json.projects.map(item => item.project), [a, b]);
  assert.deepEqual(run.json.findings.map(item => item.project), [a, b]);
  assert.equal(run.json.gaps.length, 2);
  assert.equal(run.json.projects[1].report.analysis.findings[0].project, b);
}));
test("a failing leaf of a solution fails the run and keeps the other leaves' results", () => workspace(root => {
  leaf(root, "a"); leaf(root, "b");
  writeFileSync(join(root, "tsconfig.json"), JSON.stringify({ files: [], references: [{ path: "./a" }, { path: "./b" }] }));
  const binary = fakeNative(root, `case "$2" in */b/*) echo 'b exploded' >&2; exit 9;; esac\necho '{"findings":[]}'`);
  const run = feedbackCli(["--project", join(root, "tsconfig.json")], { binary });
  assert.equal(run.status, 2);
  assert.equal(run.json.status, "native-failed");
  assert(run.json.projects[0].report && run.json.projects[1].failure.exitStatus === 9);
  assert.match(run.json.projects[1].failure.stderr.text, /b exploded/);
}));
test("a capture or manifest cannot be bound to a solution-style tsconfig", () => workspace(root => {
  const a = leaf(root, "a");
  writeFileSync(join(root, "tsconfig.json"), JSON.stringify({ files: [], references: [{ path: "./a" }] }));
  const manifest = feedbackCli(["manifest", "--project", join(root, "tsconfig.json")]);
  assert.equal(manifest.status, 2);
  assert(manifest.stderr.includes("solution-style") && manifest.stderr.includes(a), manifest.stderr);
  writeFileSync(join(root, "capture.json"), "{}");
  const binary = fakeNative(root, "echo '{\"findings\":[]}'");
  const capture = feedbackCli(["--project", join(root, "tsconfig.json"), "--capture", join(root, "capture.json")], { binary });
  assert.equal(capture.status, 2);
  assert.match(capture.stderr, /capture belongs to one project/);
}));
