import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { dirname, resolve, sep } from "node:path";
import { performance } from "node:perf_hooks";
import ts from "typescript";
import { nativeExecutable, runNative } from "../bin/launcher.mjs";

const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;

// A solution-style project (no inputs of its own, only `references`) has
// nothing to check. It is refused with the leaf projects it names so a caller
// can analyse each; the CLI expands it automatically.
export class SolutionProjectError extends Error {
  constructor(project, leaves) {
    super(`${project} is a solution-style tsconfig with no source files of its own; `
      + `analyse its referenced projects: ${leaves.join(", ") || "(none resolved)"}`);
    this.name = "SolutionProjectError";
    this.project = project;
    this.leaves = leaves;
  }
}

function parseProject(project, system = ts.sys) {
  const config = ts.readConfigFile(project, system.readFile);
  assert(!config.error, `Cannot read the TypeScript project ${project}: ${
    config.error ? ts.flattenDiagnosticMessageText(config.error.messageText, " ") : ""}`);
  return ts.parseJsonConfigFileContent(config.config, system, dirname(project), {}, project);
}

// The leaf projects of `project`, depth first and de-duplicated. A project with
// inputs of its own is its own leaf: its `references` are dependencies, not
// work to expand. A reference that does not resolve fails closed.
export function expandSolutionProject(project, seen = new Set()) {
  project = realpathSync(resolve(project));
  if (seen.has(project)) return [];
  seen.add(project);
  const parsed = parseProject(project), references = parsed.projectReferences ?? [];
  if (parsed.fileNames.length || !references.length) return [project];
  return references.flatMap(reference => {
    const path = ts.resolveProjectReferencePath(reference);
    assert(ts.sys.fileExists(path), `Referenced project not found: ${path} (referenced from ${project})`);
    return expandSolutionProject(path, seen);
  });
}

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
  const typingStarted = performance.now();
  const parsed = parseProject(project, system);
  const references = parsed.projectReferences ?? [];
  if (!parsed.fileNames.length && references.length) throw new SolutionProjectError(project, expandSolutionProject(project));
  // A project that keeps its own inputs and also lists `references` is checked
  // as the editor checks it: referenced projects contribute their *sources*
  // (no built `.d.ts` outputs are required, so no TS6305), and the native
  // checker analyses this project's files the same way. The referenced
  // tsconfigs are pinned as inputs so editing one refuses a stale report.
  const referencedProjects = references.map(reference => ts.resolveProjectReferencePath(reference));
  for (const path of referencedProjects) system.readFile(path);
  const host = ts.createCompilerHost(parsed.options);
  Object.assign(host, { readFile: system.readFile, fileExists: system.fileExists,
    directoryExists: system.directoryExists, realpath: system.realpath, getDirectories: system.getDirectories,
    getSourceFile(path, languageVersion) {
      const text = system.readFile(path);
      return text === undefined ? undefined : ts.createSourceFile(path, text, languageVersion, true);
    } });
  if (references.length) host.useSourceOfProjectReferenceRedirect = () => true;
  const program = ts.createProgram({ rootNames: parsed.fileNames, options: parsed.options, host,
    ...(references.length ? { projectReferences: references } : {}) });
  const typingErrors = [...parsed.errors, ...ts.getPreEmitDiagnostics(program)]
    .filter(item => item.category === ts.DiagnosticCategory.Error).length;
  const manifest = { project, typescript: ts.version, ...(referencedProjects.length ? { projectReferences: referencedProjects } : {}),
    inputs: [...inputs.values()] };
  return { manifest, inputId: hash(JSON.stringify(manifest)), typingErrors, sources: program.getSourceFiles(),
    sourceFileCount: parsed.fileNames.filter(name => !name.endsWith(".d.ts")).length,
    typingMs: performance.now() - typingStarted };
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

// ---------------------------------------------------------------------------
// Native child: binary selection, time limit, stderr and per-phase timings.
// ---------------------------------------------------------------------------

export const NATIVE_TIMEOUT_ENV = "SOLID_CHECKER_FEEDBACK_NATIVE_TIMEOUT";
// A release build analyses a 139-file application in about 25 s; a debug build
// is roughly 19x slower and needs minutes. Ten minutes covers a debug build of
// that application while still bounding a hung child. `0` disables the limit.
export const DEFAULT_NATIVE_TIMEOUT_MS = 600_000;
// Debug builds are only worth warning about on projects large enough to feel it.
export const DEBUG_BUILD_WARNING_SOURCE_FILES = 50;
const STDERR_RETAINED_CHARS = 16 * 1024;
const STDOUT_MAX_BYTES = 1024 ** 3;

export function nativeTimeoutMs(seconds = process.env[NATIVE_TIMEOUT_ENV]) {
  if (seconds === undefined || seconds === "") return DEFAULT_NATIVE_TIMEOUT_MS;
  const value = Number(seconds);
  assert(Number.isFinite(value) && value >= 0, `Native timeout must be a number of seconds >= 0 (0 disables the limit), got ${seconds}`);
  return Math.round(value * 1000);
}

export function describeNativeBinary(path) {
  const normalized = path.split(sep).join("/");
  const kind = /\/target\/debug\//.test(normalized) ? "debug"
    : /\/target\/release\//.test(normalized) ? "release" : "unknown";
  return { path, kind };
}

// The binary the launcher will use. An explicit override that does not exist is
// refused here: the launcher would otherwise fall back to the repository's
// `bin/` and run `make build-rust` (cargo) behind the caller's back.
function selectNativeBinary() {
  const override = process.env.SOLID_CHECKER_NATIVE_BIN;
  if (override) assert(existsSync(override), `SOLID_CHECKER_NATIVE_BIN does not exist: ${override}`);
  return describeNativeBinary(nativeExecutable("solid-checker"));
}

// Native stderr is a mix of free text (compiler warnings) and one JSON object
// per line when SOLID_CHECKER_TIMINGS is set: `{"reactiveIrStage":…}` as each
// reactive-IR stage ends and one summary object last.
export function parseNativeStderr(text) {
  const stages = [], summary = {}, kept = [];
  for (const line of (text ?? "").split("\n")) {
    const trimmed = line.trim();
    if (trimmed.startsWith("{") && trimmed.endsWith("}")) {
      try {
        const value = JSON.parse(trimmed);
        if (value && typeof value === "object" && !Array.isArray(value)) {
          if (typeof value.reactiveIrStage === "string") stages.push({ stage: value.reactiveIrStage, elapsedNs: value.elapsedNs });
          else Object.assign(summary, value);
          continue;
        }
      } catch { /* free text that looks like JSON */ }
    }
    kept.push(line);
  }
  const body = kept.join("\n").trim();
  const truncated = body.length > STDERR_RETAINED_CHARS;
  return { stages, summary: Object.keys(summary).length ? summary : null,
    text: truncated ? "…" + body.slice(-STDERR_RETAINED_CHARS) : body, truncated, bytes: Buffer.byteLength(text ?? "") };
}

// A failed native child is a structured result, not a bare string: which
// binary, how long it ran against which limit, and the last phase it reported.
export class FeedbackNativeError extends Error {
  constructor(message, failure) {
    super(message);
    this.name = "FeedbackNativeError";
    this.failure = failure;
  }
}

export function failureDocument(project, failure) {
  return { schemaVersion: 1, documentKind: "solid-checker-development-feedback", status: "native-failed",
    authority: false, certification: false, project, failure };
}

function phaseReached(stages) {
  const last = stages.at(-1)?.stage ?? null;
  return { lastCompletedStage: last, completedStages: stages.length,
    description: last ? `after reactive-IR stage "${last}"`
      : "before the first reactive-IR stage (native setup: project contract admission, source and Type Facts "
        + "acquisition, compiler facts); no stage line was emitted" };
}

function runNativeChild(args, { timeoutMs }) {
  const started = performance.now();
  // SOLID_CHECKER_TIMINGS is always on for the child so a timeout can name the
  // phase it reached; the report only carries the timings when the caller asked.
  const child = runNative("solid-checker", args, { stdio: "pipe", encoding: "utf8", killSignal: "SIGKILL",
    maxBuffer: STDOUT_MAX_BYTES, ...(timeoutMs ? { timeout: timeoutMs } : {}),
    env: { SOLID_CHECKER_DAEMON: "0", SOLID_CHECKER_TIMINGS: "1" } });
  return { ...child, wallMs: performance.now() - started };
}

function nativeFailure(project, snapshot, native, context) {
  const stderr = parseNativeStderr(native.stderr);
  const base = { project, binary: context.binary, timeoutMs: context.timeoutMs, wallMs: native.wallMs ?? null,
    exitStatus: native.status ?? null, signal: native.signal ?? null, phase: phaseReached(stderr.stages),
    stderr: { text: stderr.text, truncated: stderr.truncated, bytes: stderr.bytes },
    typingErrorCount: snapshot.typingErrors, sourceFileCount: snapshot.sourceFileCount };
  const hint = context.binary?.kind === "debug"
    ? "The native checker is a debug build (about 19x slower than release); use a release build or raise the limit." : null;
  const seconds = ms => `${(ms / 1000).toFixed(1)} s`;
  let failure;
  if (native.error?.code === "ETIMEDOUT") {
    failure = { kind: "timeout", ...base, message: `Native analysis exceeded its ${seconds(context.timeoutMs)} limit `
      + `after ${seconds(native.wallMs)}, ${base.phase.description}. Raise it with --native-timeout <seconds> or `
      + `${NATIVE_TIMEOUT_ENV} (0 disables the limit).${hint ? " " + hint : ""}`, hint };
  } else if (native.error?.code === "ENOBUFS") {
    failure = { kind: "output-too-large", ...base, message: "Native analysis output exceeded the adapter's output limit" };
  } else if (native.error) {
    failure = { kind: "spawn-error", ...base, message: `Could not run the native checker: ${native.error.message}` };
  } else if (native.signal) {
    failure = { kind: "signal", ...base, message: `Native analysis was killed by ${native.signal}, ${base.phase.description}` };
  } else {
    const tail = stderr.text ? `: ${stderr.text.split("\n").slice(-8).join("\n")}` : "";
    failure = { kind: "exit-status", ...base, message: `Native analysis exited with status ${native.status}, ${base.phase.description}${tail}` };
  }
  return new FeedbackNativeError(failure.message, failure);
}

export function inspectDevelopmentFeedback(project, { capture = null, analyze, feedbackFacts = false, snapshot: retainedSnapshot, nativeTimeoutMs: timeout, runtime = null } = {}) {
  const started = performance.now(), snapshot = retainedSnapshot ?? feedbackSnapshot(project);
  assert.equal(snapshot.manifest.project, realpathSync(resolve(project)), "Retained snapshot belongs to another project");
  const admitted = capture ? admitFeedbackCapture(snapshot, capture) : { observations: [], excluded: null };
  validateFeedbackInputs(snapshot.manifest);
  const timeoutMs = analyze ? 0 : timeout ?? nativeTimeoutMs();
  const binary = analyze ? null : selectNativeBinary();
  const warnings = [];
  if (binary?.kind === "debug" && (snapshot.sourceFileCount ?? 0) >= DEBUG_BUILD_WARNING_SOURCE_FILES) {
    warnings.push(`The native checker is a debug build (${binary.path}): analysis of this ${snapshot.sourceFileCount}-file project `
      + "is roughly 19x slower than a release build. Point SOLID_CHECKER_NATIVE_BIN or --native-bin at a release binary.");
  }
  const native = analyze ? analyze(snapshot.manifest.project) : runNativeChild([
    "--project", snapshot.manifest.project, "--format", "json", ...(feedbackFacts ? ["--feedback-facts"] : []),
    // The runtime the caller executes: `feedback run` serves a client-rendered
    // development build to Chromium, so package claims are answered for that
    // runtime rather than for every host.
    ...(runtime ? ["--runtime-target", runtime.target, "--runtime-build", runtime.build, "--rendering", runtime.rendering] : [])
  ], { timeoutMs });
  const context = { binary, timeoutMs };
  if (native.error || native.signal || ![0, 1].includes(native.status)) throw nativeFailure(snapshot.manifest.project, snapshot, native, context);
  let analysis;
  try { analysis = JSON.parse(native.stdout); } catch (cause) {
    const failure = nativeFailure(snapshot.manifest.project, snapshot, { ...native, status: native.status }, context).failure;
    throw new FeedbackNativeError(`Native analysis produced malformed JSON (${cause.message})`,
      { ...failure, kind: "malformed-output", message: `Native analysis produced malformed JSON (${cause.message})`,
        stdoutHead: String(native.stdout).slice(0, 200) });
  }
  assert(Array.isArray(analysis.findings), "Native response has no findings");
  validateFeedbackInputs(snapshot.manifest);
  if (capture) admitFeedbackCapture(snapshot, capture);
  const stderr = parseNativeStderr(native.stderr);
  const timingsRequested = !["", "0", undefined].includes(process.env.SOLID_CHECKER_TIMINGS);
  return { schemaVersion: 1, documentKind: "solid-checker-development-feedback",
    authority: false, certification: false, project: snapshot.manifest.project,
    inputId: snapshot.inputId, analysis, nativeExitCode: native.status, ...admitted, typingErrorCount: snapshot.typingErrors,
    findings: analysis.findings.filter(finding => finding.kind === "violation"),
    gaps: analysis.findings.filter(finding => finding.kind === "uncertifiable"),
    coverage: { runtime: capture ? "supplied-records-only" : "unavailable", packageInference: "unavailable" },
    native: { binary, timeoutMs, wallMs: native.wallMs ?? null, sourceFileCount: snapshot.sourceFileCount ?? null,
      stderr: { text: stderr.text, truncated: stderr.truncated, bytes: stderr.bytes },
      ...(timingsRequested ? { timings: { stages: stderr.stages, summary: stderr.summary } } : {}) },
    ...(snapshot.manifest.projectReferences ? { projectReferences: snapshot.manifest.projectReferences } : {}),
    ...(warnings.length ? { warnings } : {}),
    typingMs: snapshot.typingMs ?? null,
    durationMs: performance.now() - started };
}

// `feedback --project` entry: a solution-style tsconfig is expanded to its
// referenced leaf projects and each is analysed in its own native session.
// One leaf failing is recorded against that leaf and fails the whole run.
export function inspectFeedbackProjects(project, options = {}) {
  try {
    return inspectDevelopmentFeedback(project, options);
  } catch (error) {
    if (!(error instanceof SolutionProjectError)) throw error;
    assert(!options.capture, "A capture belongs to one project's inputs; pass a referenced project to --capture. "
      + `Referenced projects: ${error.leaves.join(", ")}`);
    assert(error.leaves.length, `${error.project} is a solution-style tsconfig whose references resolve to no projects`);
    const started = performance.now(), projects = [];
    for (const leaf of error.leaves) {
      try { projects.push({ project: leaf, report: inspectDevelopmentFeedback(leaf, options) }); }
      catch (failure) {
        if (!(failure instanceof FeedbackNativeError)) throw failure;
        projects.push({ project: leaf, failure: failure.failure });
      }
    }
    const reports = projects.flatMap(item => item.report ? [item.report] : []);
    const failed = projects.some(item => item.failure);
    return { schemaVersion: 1, documentKind: "solid-checker-development-feedback", authority: false, certification: false,
      project: error.project, solution: { referencedProjects: error.leaves }, projects,
      ...(failed ? { status: "native-failed" } : {}),
      findings: reports.flatMap(report => report.findings), gaps: reports.flatMap(report => report.gaps),
      observations: [], typingErrorCount: reports.reduce((sum, report) => sum + report.typingErrorCount, 0),
      nativeExitCode: failed ? 2 : Math.max(0, ...reports.map(report => report.nativeExitCode)),
      coverage: { runtime: "unavailable", packageInference: "unavailable" },
      ...(reports.some(report => report.warnings) ? { warnings: [...new Set(reports.flatMap(report => report.warnings ?? []))] } : {}),
      durationMs: performance.now() - started };
  }
}

export const developmentFeedbackHelp = `Development feedback (experimental)
  solid-checker feedback --project <tsconfig.json> [--capture <capture.json>]
    [--native-timeout <seconds>] [--native-bin <path>]
  solid-checker feedback manifest --project <tsconfig.json>
  solid-checker feedback run --project <tsconfig.json> --scenario <scenario.json>
    --browser <chromium-path> [--tooling <directory>] [--compare-project <tsconfig.json>]

Returns native checker findings and separately labeled recorded observations.
Read observations are information; they do not prove a stale result.
Live runs also report conditional automatic warnings from native result models.
Scenarios may contain interactions without assertions or comparison projects.
The manifest command creates a capture template before application execution.
A solution-style tsconfig (references only) is expanded to its referenced projects.
--native-timeout (default 600 s, 0 = no limit; also ${NATIVE_TIMEOUT_ENV}) bounds each
native analysis; --native-bin selects the native checker (also SOLID_CHECKER_NATIVE_BIN).
Use a release build: a debug build is roughly 19x slower. SOLID_CHECKER_TIMINGS=1 adds
the native per-phase timings to the report.
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
    assert((live ? ["--project", "--scenario", "--browser", "--tooling", "--compare-project", "--native-timeout", "--native-bin"]
      : ["--project", "--capture", "--native-timeout", "--native-bin"]).includes(name), `Unknown feedback option ${name}`);
    assert(args[index + 1] && !args[index + 1].startsWith("--"), `${name} requires a value`);
    assert(!options.has(name), `Duplicate feedback option ${name}`);
    options.set(name, args[index + 1]);
    if (name === "--project") { assert(!project, "Duplicate project option"); project = args[++index]; }
    else if (name === "--capture") { assert(!capture, "Duplicate capture option"); capture = args[++index]; }
    else index++;
  }
  assert(project, "Feedback requires --project");
  assert(!manifest || !capture, "Capture manifest does not accept --capture");
  // Both reach the native child through the environment, so the browser
  // collector's own native call inherits them without a signature change.
  if (options.has("--native-timeout")) {
    nativeTimeoutMs(options.get("--native-timeout"));
    process.env[NATIVE_TIMEOUT_ENV] = options.get("--native-timeout");
  }
  if (options.has("--native-bin")) process.env.SOLID_CHECKER_NATIVE_BIN = resolve(options.get("--native-bin"));
  try {
    if (live) {
      assert(options.has("--scenario") && options.has("--browser"), "Browser feedback requires --scenario and --browser");
      const { runBrowserFeedback } = await import("./feedback-browser.mjs");
      const result = await runBrowserFeedback({ project, scenarioPath: options.get("--scenario"),
        browserPath: options.get("--browser"), toolingRoot: options.get("--tooling"), comparisonProject: options.get("--compare-project") });
      process.stdout.write(JSON.stringify(result, null, 2) + "\n"); process.exitCode = result.nativeExitCode; return;
    }
    const result = manifest ? captureTemplate(feedbackSnapshot(project))
      : inspectFeedbackProjects(project, { capture: capture ? JSON.parse(readFileSync(capture, "utf8")) : null });
    process.stdout.write(JSON.stringify(result, null, 2) + "\n");
    for (const warning of result.warnings ?? []) process.stderr.write(`solid-checker: warning: ${warning}\n`);
    if (!manifest) process.exitCode = result.nativeExitCode;
  } catch (error) {
    if (!(error instanceof FeedbackNativeError)) throw error;
    // The partial result is the structured failure; the message is for humans.
    process.stdout.write(JSON.stringify(failureDocument(realpathSync(resolve(project)), error.failure), null, 2) + "\n");
    throw error;
  }
}
