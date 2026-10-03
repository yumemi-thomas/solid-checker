import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, realpathSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import ts from "typescript";
import { runNative } from "../bin/launcher.mjs";

const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;

// TypeScript is the published-typing oracle here. Semantic analysis remains in
// the native checker and its Type Facts/compiler fact domains.
export function feedbackSnapshot(project) {
  project = realpathSync(resolve(project));
  const inputs = new Map();
  const record = input => inputs.set(JSON.stringify([input.kind, input.path, input.arguments]), input);
  const system = { ...ts.sys };
  for (const method of ["readFile", "fileExists", "directoryExists", "realpath", "readDirectory", "getDirectories"]) {
    const original = ts.sys[method];
    if (!original) continue;
    system[method] = (...args) => {
      const result = original(...args);
      record({ kind: method, path: resolve(args[0]), arguments: args.slice(1).map(value => value ?? null),
        result: method === "readFile" ? result === undefined ? null : hash(result) : result });
      return result;
    };
  }
  const config = ts.readConfigFile(project, system.readFile);
  assert(!config.error, "Cannot read the TypeScript project");
  const parsed = ts.parseJsonConfigFileContent(config.config, system, dirname(project), {}, project);
  assert(!parsed.projectReferences?.length, "Feedback project references need explicit project sessions");
  const host = ts.createCompilerHost(parsed.options);
  Object.assign(host, { readFile: system.readFile, fileExists: system.fileExists,
    directoryExists: system.directoryExists, realpath: system.realpath, getDirectories: system.getDirectories,
    getSourceFile(path, languageVersion) {
      const text = system.readFile(path);
      return text === undefined ? undefined : ts.createSourceFile(path, text, languageVersion, true);
    } });
  const program = ts.createProgram(parsed.fileNames, parsed.options, host);
  const typingErrors = [...parsed.errors, ...ts.getPreEmitDiagnostics(program)]
    .filter(item => item.category === ts.DiagnosticCategory.Error).length;
  const manifest = { project, typescript: ts.version, inputs: [...inputs.values()] };
  return { manifest, inputId: hash(JSON.stringify(manifest)), typingErrors, sources: program.getSourceFiles() };
}

export function validateFeedbackInputs(manifest) {
  for (const input of manifest.inputs) {
    const read = ts.sys[input.kind];
    assert(["readFile", "fileExists", "directoryExists", "realpath", "readDirectory", "getDirectories"].includes(input.kind)
      && typeof read === "function", "Unsupported feedback input");
    const current = read(input.path, ...input.arguments.map(value => value ?? undefined));
    const result = input.kind === "readFile" ? current === undefined ? null : hash(current) : current;
    assert.deepEqual(result, input.result, `Feedback input changed: ${input.path}`);
  }
}

export function captureTemplate(snapshot) {
  return { schemaVersion: 1, documentKind: "solid-checker-development-capture",
    project: snapshot.manifest.project, inputId: snapshot.inputId,
    manifest: snapshot.manifest, runtimeInputs: [], events: [],
    authority: false, certification: false };
}

export function admitFeedbackCapture(snapshot, capture) {
  assert.equal(capture.schemaVersion, 1);
  assert.equal(capture.documentKind, "solid-checker-development-capture");
  assert.equal(capture.authority, false); assert.equal(capture.certification, false);
  assert.equal(capture.project, snapshot.manifest.project);
  assert.equal(capture.inputId, snapshot.inputId, "Capture belongs to different project inputs");
  assert.deepEqual(capture.manifest, snapshot.manifest, "Capture input manifest differs");
  assert(Array.isArray(capture.runtimeInputs) && Array.isArray(capture.events));
  validateFeedbackInputs(capture.manifest);
  const runtime = new Map();
  for (const input of capture.runtimeInputs) {
    assert.equal(realpathSync(input.path), input.path, "Runtime input path is not canonical");
    assert.equal(hash(readFileSync(input.path)), input.sha256, "Runtime input changed");
    assert(!runtime.has(input.path), "Duplicate runtime input"); runtime.set(input.path, input.sha256);
  }
  const sources = new Map(snapshot.sources.map(source => [resolve(source.fileName), source]));
  const seen = new Set(), observations = [], groups = new Map();
  for (const event of capture.events) {
    assert(typeof event.id === "string" && event.id.length && !seen.has(event.id), "Duplicate or missing capture event identity");
    seen.add(event.id);
    assert(["runtime-exception", "untracked-read", "observer-query", "assertion-failure"].includes(event.kind), "Unsupported capture event");
    assert(typeof event.message === "string" && event.message.trim(), "Capture event message is missing");
    const location = event.location, source = sources.get(location?.path);
    assert(source && !source.isDeclarationFile, "Capture location is outside analyzed source");
    assert.equal(event.sourceSha256, hash(source.text), "Captured source changed");
    const bytes = Buffer.from(source.text);
    assert(Number.isSafeInteger(location.startByte) && Number.isSafeInteger(location.endByte)
      && location.startByte >= 0 && location.endByte > location.startByte && location.endByte <= bytes.length,
      "Invalid capture source span");
    assert.equal(Buffer.byteLength(bytes.subarray(0, location.startByte).toString("utf8")), location.startByte,
      "Capture span cuts a UTF-8 code point");
    assert.equal(Buffer.byteLength(bytes.subarray(0, location.endByte).toString("utf8")), location.endByte,
      "Capture span cuts a UTF-8 code point");
    if (["untracked-read", "observer-query"].includes(event.kind)) {
      assert(event.tracking === "untracked" && event.runtimeInput && runtime.has(event.runtimeInput),
        "Read observation needs explicit tracking state and bound runtime bytes");
    }
    // These are supplied development observations, never static violations.
    // Intent-sensitive read records remain information, including snapshots.
    const key = JSON.stringify([event.kind, location, event.message, event.runtimeInput]);
    if (groups.has(key)) {
      groups.get(key).eventIds.push(event.id);
      groups.get(key).occurrences++;
      continue;
    }
    const observation = { id: event.id, eventIds: [event.id], occurrences: 1,
      channel: "recorded-runtime", kind: event.kind,
      severity: ["untracked-read", "observer-query"].includes(event.kind) ? "info" : "error", message: event.message,
      location, sourceSha256: event.sourceSha256, inputId: capture.inputId,
      reactiveIntent: "open", authority: false, certification: false };
    observations.push(observation); groups.set(key, observation);
  }
  return snapshot.typingErrors ? { observations: [], excluded: "TypeScript owns this input" }
    : { observations, excluded: null };
}

export function inspectDevelopmentFeedback(project, { capture = null, analyze, feedbackFacts = false, snapshot: retainedSnapshot } = {}) {
  const started = performance.now(), snapshot = retainedSnapshot ?? feedbackSnapshot(project);
  assert.equal(snapshot.manifest.project, realpathSync(resolve(project)), "Retained snapshot belongs to another project");
  const admitted = capture ? admitFeedbackCapture(snapshot, capture) : { observations: [], excluded: null };
  validateFeedbackInputs(snapshot.manifest);
  const native = analyze ? analyze(snapshot.manifest.project) : runNative("solid-checker", [
    "--project", snapshot.manifest.project, "--format", "json", ...(feedbackFacts ? ["--feedback-facts"] : [])
  ], { stdio: "pipe", encoding: "utf8", env: { SOLID_CHECKER_DAEMON: "0" } });
  if (native.error) throw native.error;
  assert([0, 1].includes(native.status), native.stderr || "Native analysis failed");
  const analysis = JSON.parse(native.stdout);
  assert(Array.isArray(analysis.findings), "Native response has no findings");
  validateFeedbackInputs(snapshot.manifest);
  if (capture) admitFeedbackCapture(snapshot, capture);
  return { schemaVersion: 1, documentKind: "solid-checker-development-feedback",
    authority: false, certification: false, project: snapshot.manifest.project,
    inputId: snapshot.inputId, analysis, nativeExitCode: native.status, ...admitted, typingErrorCount: snapshot.typingErrors,
    findings: analysis.findings.filter(finding => finding.kind === "violation"),
    gaps: analysis.findings.filter(finding => finding.kind === "uncertifiable"),
    coverage: { runtime: capture ? "supplied-records-only" : "unavailable", packageInference: "unavailable" },
    durationMs: performance.now() - started };
}

export const developmentFeedbackHelp = `Development feedback (experimental)
  solid-checker feedback --project <tsconfig.json> [--capture <capture.json>]
  solid-checker feedback manifest --project <tsconfig.json>
  solid-checker feedback run --project <tsconfig.json> --scenario <scenario.json>
    --browser <chromium-path> [--tooling <directory>] [--compare-project <tsconfig.json>]

Returns native checker findings and separately labeled recorded observations.
Read observations are information; they do not prove a stale result.
Live runs also report conditional automatic warnings from native result models.
Scenarios may contain interactions without assertions or comparison projects.
The manifest command creates a capture template before application execution.
`;

export async function developmentFeedback(args) {
  if (!args.length || args.includes("--help") || args.includes("-h")) {
    process.stdout.write(developmentFeedbackHelp); return;
  }
  const manifest = args[0] === "manifest", live = args[0] === "run";
  if (manifest || live) args = args.slice(1);
  let project, capture;
  const options = new Map();
  for (let index = 0; index < args.length; index++) {
    const name = args[index];
    assert((live ? ["--project", "--scenario", "--browser", "--tooling", "--compare-project"] : ["--project", "--capture"]).includes(name), `Unknown feedback option ${name}`);
    assert(args[index + 1] && !args[index + 1].startsWith("--"), `${name} requires a path`);
    assert(!options.has(name), `Duplicate feedback option ${name}`);
    options.set(name, args[index + 1]);
    if (name === "--project") { assert(!project, "Duplicate project option"); project = args[++index]; }
    else if (name === "--capture") { assert(!capture, "Duplicate capture option"); capture = args[++index]; }
    else index++;
  }
  assert(project, "Feedback requires --project");
  assert(!manifest || !capture, "Capture manifest does not accept --capture");
  if (live) {
    assert(options.has("--scenario") && options.has("--browser"), "Browser feedback requires --scenario and --browser");
    const { runBrowserFeedback } = await import("./feedback-browser.mjs");
    const result = await runBrowserFeedback({ project, scenarioPath: options.get("--scenario"),
      browserPath: options.get("--browser"), toolingRoot: options.get("--tooling"), comparisonProject: options.get("--compare-project") });
    process.stdout.write(JSON.stringify(result, null, 2) + "\n"); process.exitCode = result.nativeExitCode; return;
  }
  const result = manifest ? captureTemplate(feedbackSnapshot(project))
    : inspectDevelopmentFeedback(project, { capture: capture ? JSON.parse(readFileSync(capture, "utf8")) : null });
  process.stdout.write(JSON.stringify(result, null, 2) + "\n");
  if (!manifest) process.exitCode = result.nativeExitCode;
}
