#!/usr/bin/env bun
// Differential harness for any two Type Facts producer binaries: record what
// the checker asked a REFERENCE producer, replay the same request stream
// against a CANDIDATE producer, decode both answers with the Rust client
// decoder and diff the rows. The reference is --reference BIN, else
// SOLID_TYPEFACTS_REFERENCE_BIN, else the archived Go producer
// ($TSGO_ARCHIVE/bin/solid-typefacts-go, see docs/typefacts/differential.md);
// the candidate is --candidate BIN, else SOLID_TYPEFACTS_CANDIDATE_BIN, else
// bin/solid-typefacts. (Transcript files and report cells keep the historical
// names: the reference side is "go"/responses.bin, the candidate "rust".)
//
//   record   run a gate (or any command) with every producer session captured
//   replay   replay each recorded request stream against the candidate producer
//   compare  decode both response streams, normalise, diff, report
//   run      replay + compare
//   e2e      `coverage` with the checker on the candidate producer; compare-only
//
// Usage (see docs/typefacts/differential.md):
//   bun scripts/typefacts-differential.mjs record  --out DIR [--reference BIN] [--gate NAME] [--label L] [-- args | command...]
//   bun scripts/typefacts-differential.mjs replay  DIR [--candidate BIN] [--reference BIN] [--jobs N]
//   bun scripts/typefacts-differential.mjs compare DIR [--report FILE] [--examples N]
//        [--allow-list FILE] [--allow-unavailable] [--jobs N]
//   replay also takes --canonicalize-requests (diagnostic: send canonical CBOR key order)
//   and --no-evidence (skip the declarations pass that proves library-only durable ids)
//   bun scripts/typefacts-differential.mjs run     DIR [same options]
//
// Exit status of compare/run: 0 when every cell is equal or allow-listed (and,
// with --allow-unavailable, when the only other cells are rust-unavailable
// ones); 1 otherwise; 2 on a usage or environment error.

import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { gateConcurrency, mapPool } from "./lib/pool.mjs";
import {
  applyAllowList,
  buildReport,
  diffTranscripts,
  loadAllowList,
  normalizeTranscript,
  summarize,
  summarizeCoverage,
  summarizeCoverageText
} from "./lib/typefacts-differential.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_ALLOW_LIST = join(ROOT, "scripts/typefacts-differential-allowlist.json");
const PROXY = join(ROOT, "scripts/typefacts-transcript-proxy.mjs");
const TOOL_PROFILE = "debug";

const ARCHIVED_GO_RELATIVE = "bin/solid-typefacts-go";
const defaultArchive = () =>
  process.env.TSGO_ARCHIVE ??
  join(process.env.HOME ?? "", "Documents/Github/sc-worktrees.noindex/tsgo-archive");

/** The reference producer: --reference (alias --go), env, else the archived Go binary. */
export function referenceBinary(options, env = process.env) {
  const path = resolve(
    options.reference ?? options.go ?? env.SOLID_TYPEFACTS_REFERENCE_BIN ?? join(defaultArchive(), ARCHIVED_GO_RELATIVE)
  );
  if (!existsSync(path)) {
    console.error(
      `no reference producer at ${path}\n` +
        "  pass --reference BIN or set SOLID_TYPEFACTS_REFERENCE_BIN to a producer binary, or set TSGO_ARCHIVE\n" +
        "  to the archived Go producer's directory (the default reference is $TSGO_ARCHIVE/bin/solid-typefacts-go)"
    );
    process.exit(2);
  }
  return path;
}

/** The candidate producer: --candidate (alias --rust), env, else bin/solid-typefacts. */
export function candidateBinary(options, env = process.env) {
  return requireFile(
    resolve(options.candidate ?? options.rust ?? env.SOLID_TYPEFACTS_CANDIDATE_BIN ?? join(ROOT, "bin/solid-typefacts")),
    "run `make build-typefacts`, or pass --candidate BIN"
  );
}

/** Gates that can be recorded; each runs with SOLID_TYPEFACTS_BIN at the proxy. */
export const GATES = {
  coverage: { command: ["bun", "scripts/coverage.mjs"] },
  "ownership-gate": {
    command: ["bun", "scripts/ownership-gate.mjs", "--require-retained", "--require-complete"]
  },
  "tsc-oracle-gate": { command: ["bun", "scripts/tsc-oracle-gate.mjs"] },
  "obligation-audit": { command: ["bun", "scripts/obligation-audit.mjs"] },
  // The sweep takes its producer as a flag, not from the environment.
  "corpus-sweep": { command: ["node", "scripts/corpus-sweep.mjs"], producerFlag: "--typefacts" }
};

export function parseArguments(argv) {
  const positional = [];
  const options = {};
  const rest = [];
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--") {
      rest.push(...argv.slice(index + 1));
      break;
    }
    if (argument.startsWith("--")) {
      const name = argument.slice(2);
      if (["allow-unavailable", "reuse", "quiet", "canonicalize-requests", "no-evidence"].includes(name)) options[name] = true;
      else {
        options[name] = argv[index + 1];
        if (options[name] === undefined) throw new Error(`${argument} needs a value`);
        index += 1;
      }
    } else positional.push(argument);
  }
  return { positional, options, rest };
}

const toolPath = () =>
  process.env.TYPEFACTS_TRANSCRIPT_BIN ?? join(ROOT, "rust/target", TOOL_PROFILE, "typefacts-transcript");

function requireFile(path, hint) {
  if (!existsSync(path)) {
    console.error(`missing ${path} -- ${hint}`);
    process.exit(2);
  }
  return path;
}

/** Every recorded session directory (one holding requests.bin) under `directory`. */
export function findSessions(directory) {
  const found = [];
  const walk = (current) => {
    if (existsSync(join(current, "requests.bin"))) {
      found.push(current);
      return;
    }
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      if (entry.isDirectory()) walk(join(current, entry.name));
    }
  };
  walk(directory);
  return found.sort();
}

const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));

/** The `-project` argument of a recorded producer invocation, relative to the repo. */
export function projectOf(session) {
  try {
    const argv = readJson(join(session, "argv.json"));
    const index = argv.indexOf("-project");
    if (index === -1) return relative(ROOT, session);
    const project = argv[index + 1];
    return project.startsWith(ROOT) ? relative(ROOT, project) : project;
  } catch {
    return relative(ROOT, session);
  }
}

function capture(command, args, options = {}) {
  return new Promise((resolveRun) => {
    const child = spawn(command, args, { ...options, stdio: ["ignore", "pipe", "pipe"] });
    const out = [];
    const err = [];
    child.stdout.on("data", (chunk) => out.push(chunk));
    child.stderr.on("data", (chunk) => err.push(chunk));
    child.on("close", (status) =>
      resolveRun({ status, stdout: Buffer.concat(out), stderr: Buffer.concat(err).toString() })
    );
  });
}

// --- record ----------------------------------------------------------------

export function recordEnvironment({ out, label, target, base = process.env }) {
  return {
    ...base,
    SOLID_TYPEFACTS_BIN: PROXY,
    TYPEFACTS_TRANSCRIPT_TARGET: target,
    TYPEFACTS_TRANSCRIPT_DIR: resolve(out),
    TYPEFACTS_TRANSCRIPT_LABEL: label,
    // A cached unit would replay a result instead of running the producer, so
    // nothing would be recorded for it.
    SOLID_CHECKER_GATE_CACHE: "0",
    SOLID_CHECKER_DAEMON: "0"
  };
}

function record({ options, rest }) {
  if (!options.out) throw new Error("record needs --out DIR");
  const target = referenceBinary(options);
  const gate = options.gate ? GATES[options.gate] : null;
  if (options.gate && !gate) throw new Error(`unknown gate ${options.gate}; one of ${Object.keys(GATES)}`);
  let command;
  if (gate) {
    command = [...gate.command, ...rest];
    if (gate.producerFlag && !rest.includes(gate.producerFlag)) command.push(gate.producerFlag, PROXY);
  } else {
    command = rest;
  }
  if (command.length === 0) throw new Error("record needs --gate NAME or a command after --");
  const out = resolve(options.out, options.gate ?? "");
  mkdirSync(out, { recursive: true });
  const env = recordEnvironment({ out, label: options.label ?? options.gate ?? "", target });
  if (!env.SOLID_CHECKER_BIN) {
    const debug = join(ROOT, "rust/target/debug/solid-checker-rust");
    if (existsSync(debug)) env.SOLID_CHECKER_BIN = debug;
  }
  console.error(`recording ${command.join(" ")} -> ${out}`);
  const result = spawnSync(command[0], command.slice(1), { cwd: ROOT, env, stdio: "inherit" });
  const sessions = findSessions(out).length;
  console.error(`recorded ${sessions} producer session(s) in ${out} (command exit ${result.status})`);
  return result.status === 0 ? 0 : 1;
}

// --- replay ----------------------------------------------------------------

/**
 * The recorded argv with the Rust producer as argv[0] and the checker's
 * temporary `-project-config` file replaced by the copy the proxy kept.
 */
export function replayArguments(argv, rust, session, { hasConfigCopy = existsSync(join(session, "project-config.bin")) } = {}) {
  const copy = join(session, "project-config.bin");
  const args = argv.slice(1).map((arg, index, all) => {
    if (!hasConfigCopy) return arg;
    if (arg.startsWith("-project-config=")) return `-project-config=${copy}`;
    if (all[index - 1] === "-project-config") return copy;
    return arg;
  });
  return [rust, ...args];
}

/**
 * The evidence pass for the reference producer: the same recorded requests
 * replayed against it, only to collect its own answer to "declare these ids".
 * Its responses are discarded (the recording is already its answer). The
 * binary is the resolved reference, not the recorded argv[0]: a recording
 * names whatever sat at bin/solid-typefacts then.
 */
async function goEvidence(session, { tool, meta, reference }) {
  const argv = readJson(join(session, "argv.json"));
  writeFileSync(join(session, "argv-go.json"), JSON.stringify(replayArguments(argv, reference, session)));
  await capture(
    tool,
    [
      "replay",
      join(session, "requests.bin"),
      join(session, "responses.bin"),
      join(session, "argv-go.json"),
      join(session, "go-replay-responses.bin")
    ],
    {
      cwd: meta.cwd ?? ROOT,
      env: {
        ...process.env,
        SOLID_TYPEFACTS_TIMINGS: "",
        TYPEFACTS_REPLAY_EVIDENCE: join(session, "go-evidence.json")
      }
    }
  );
}

async function replayOne(session, { rust, reference, tool, options }) {
  const argv = readJson(join(session, "argv.json"));
  writeFileSync(join(session, "argv-rust.json"), JSON.stringify(replayArguments(argv, rust, session)));
  const meta = existsSync(join(session, "meta.json")) ? readJson(join(session, "meta.json")) : {};
  const evidence = !options["no-evidence"];
  const run = await capture(
    tool,
    [
      "replay",
      join(session, "requests.bin"),
      join(session, "responses.bin"),
      join(session, "argv-rust.json"),
      join(session, "rust-responses.bin")
    ],
    {
      cwd: meta.cwd ?? ROOT,
      env: {
        ...process.env,
        SOLID_TYPEFACTS_TIMINGS: "",
        ...(evidence ? { TYPEFACTS_REPLAY_EVIDENCE: join(session, "rust-evidence.json") } : {}),
        ...(options["canonicalize-requests"] ? { TYPEFACTS_REPLAY_CANONICALIZE: "1" } : {})
      }
    }
  );
  if (evidence) await goEvidence(session, { tool, meta, reference });
  let report;
  try {
    report = JSON.parse(run.stdout.toString());
  } catch {
    report = { stopped: `replay tool failed (${run.status}): ${run.stderr.trim()}` };
  }
  writeFileSync(join(session, "replay.json"), JSON.stringify({ ...report, stderr: run.stderr.trim() }, null, 2));
  return report;
}

async function replay({ positional, options }) {
  const directory = resolve(positional[0] ?? "");
  const rust = candidateBinary(options);
  const reference = options["no-evidence"] ? null : referenceBinary(options);
  const tool = requireFile(toolPath(), "run `make build-typefacts-transcript`");
  const sessions = findSessions(directory);
  if (sessions.length === 0) throw new Error(`no recorded sessions under ${directory}`);
  const todo = options.reuse
    ? sessions.filter((session) => !existsSync(join(session, "rust-responses.bin")))
    : sessions;
  const concurrency = options.jobs ? Number(options.jobs) : gateConcurrency();
  let stopped = 0;
  await mapPool(todo, async (session) => {
    const report = await replayOne(session, { rust, reference, tool, options });
    if (report.stopped) stopped += 1;
  }, { concurrency });
  console.error(`replayed ${todo.length}/${sessions.length} session(s) against ${rust} (reference for evidence: ${reference ?? "none"}); ${stopped} stopped early`);
  return 0;
}

// --- compare ---------------------------------------------------------------

async function decode(tool, requests, responses) {
  const run = await capture(tool, ["decode", requests, responses], {});
  if (run.status !== 0) throw new Error(run.stderr.trim() || `decode exited ${run.status}`);
  return JSON.parse(run.stdout.toString());
}

/** The symbol rows of an evidence file; none when the pass did not run or failed. */
function evidenceRows(path) {
  try {
    const rows = readJson(path).rows;
    return Array.isArray(rows) ? rows : [];
  } catch {
    return [];
  }
}

/** Compares one recorded session; never throws (a failure is a `transport` result). */
export async function compareSession(session, tool) {
  const project = projectOf(session);
  const meta = { project, session: relative(ROOT, session) };
  const requests = join(session, "requests.bin");
  const failure = (reason) => ({
    results: [
      {
        ...meta,
        index: -1,
        operation: "session",
        family: "session",
        column: "replay",
        key: "",
        category: "transport",
        reason
      }
    ],
    failed: { session: meta.session, reason }
  });
  let go;
  try {
    go = normalizeTranscript(
      await decode(tool, requests, join(session, "responses.bin")),
      evidenceRows(join(session, "go-evidence.json"))
    );
  } catch (error) {
    return failure(`cannot decode the Go transcript: ${error.message}`);
  }
  const rustPath = join(session, "rust-responses.bin");
  if (!existsSync(rustPath)) return failure("no rust-responses.bin (run replay first)");
  let rust;
  try {
    rust = normalizeTranscript(
      await decode(tool, requests, rustPath),
      evidenceRows(join(session, "rust-evidence.json"))
    );
  } catch (error) {
    return failure(`cannot decode the Rust transcript: ${error.message}`);
  }
  const replayReport = existsSync(join(session, "replay.json")) ? readJson(join(session, "replay.json")) : {};
  const replayNote = [replayReport.stopped, replayReport.stderr].filter(Boolean).join("; ");
  const results = diffTranscripts(go, rust, { ...meta, replayNote });
  const failed = replayReport.stopped ? { session: meta.session, reason: replayReport.stopped } : null;
  const operations = {};
  for (const exchange of go.exchanges) {
    operations[exchange.operation] = (operations[exchange.operation] ?? 0) + 1;
  }
  return { results, failed, operations };
}

async function compare({ positional, options }) {
  const directory = resolve(positional[0] ?? "");
  const tool = requireFile(toolPath(), "run `make build-typefacts-transcript`");
  const sessions = findSessions(directory);
  if (sessions.length === 0) throw new Error(`no recorded sessions under ${directory}`);
  const allowPath = options["allow-list"] ?? DEFAULT_ALLOW_LIST;
  const entries = existsSync(allowPath) ? loadAllowList(allowPath) : [];
  const concurrency = options.jobs ? Number(options.jobs) : gateConcurrency();
  const per = await mapPool(sessions, (session) => compareSession(session, tool), { concurrency });
  const results = per.flatMap((entry) => entry.results);
  const allowUsed = applyAllowList(results, entries);
  const failedSessions = per.map((entry) => entry.failed).filter(Boolean);
  const unavailableOperations = {};
  for (const result of results) {
    if (result.category === "rust-unavailable") {
      unavailableOperations[result.operation] = (unavailableOperations[result.operation] ?? 0) + 1;
    }
  }
  const report = buildReport(results, {
    limit: options.examples ? Number(options.examples) : 3,
    sessions: sessions.length,
    failedSessions,
    allowUsed,
    unavailableOperations
  });
  const reportPath = resolve(options.report ?? join(directory, "differential-report.json"));
  writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  if (!options.quiet) console.log(summarize(report));
  console.log(`report: ${reportPath}`);
  const unmet = report.failing + (options["allow-unavailable"] ? 0 : report.unavailable);
  return unmet === 0 && failedSessions.length === 0 ? 0 : 1;
}

// --- end-to-end ------------------------------------------------------------

const snapshotState = () =>
  spawnSync("git", ["status", "--porcelain", "--", "fixtures/findings-snapshots"], {
    cwd: ROOT,
    encoding: "utf8"
  }).stdout;

/**
 * `make coverage` with the checker pointed at the Rust producer, compare-only.
 * coverage.mjs refuses --update under SOLID_COVERAGE_REPORT, and the snapshot
 * directory is checked against git before and after as a second guard.
 */
function e2e({ options }) {
  const rust = candidateBinary(options);
  const checker = process.env.SOLID_CHECKER_BIN ?? join(ROOT, "rust/target/debug/solid-checker-rust");
  requireFile(checker, "run `make build-checker-debug`");
  const out = resolve(options.out ?? join(ROOT, "rust/target/differential/e2e"));
  mkdirSync(out, { recursive: true });
  const reportPath = join(out, "coverage-moved.json");
  rmSync(reportPath, { force: true });
  const before = snapshotState();
  const result = spawnSync("bun", ["scripts/coverage.mjs"], {
    cwd: ROOT,
    stdio: ["ignore", "ignore", "ignore"],
    env: {
      ...process.env,
      SOLID_CHECKER_BIN: checker,
      SOLID_TYPEFACTS_BIN: rust,
      SOLID_COVERAGE_REPORT: reportPath,
      SOLID_CHECKER_GATE_CACHE: "0"
    }
  });
  if (snapshotState() !== before) {
    console.error("snapshots changed during the end-to-end run; this mode must never write them");
    return 3;
  }
  if (!existsSync(reportPath) || result.status === 2) {
    console.error(`coverage did not produce ${reportPath} (exit ${result.status})`);
    return 2;
  }
  const summary = summarizeCoverage(readJson(reportPath));
  writeFileSync(join(out, "summary.json"), `${JSON.stringify(summary, null, 2)}\n`);
  console.log(summarizeCoverageText(summary));
  console.log(`reports: ${reportPath}, ${join(out, "summary.json")}`);
  return summary.states.unchanged === summary.projects ? 0 : 1;
}

async function main() {
  const { positional, options, rest } = parseArguments(process.argv.slice(2));
  const [verb, ...tail] = positional;
  const args = { positional: tail, options, rest };
  switch (verb) {
    case "record":
      return record(args);
    case "replay":
      return replay(args);
    case "compare":
      return compare(args);
    case "e2e":
      return e2e(args);
    case "run": {
      await replay(args);
      return compare(args);
    }
    default:
      console.error("usage: typefacts-differential.mjs record|replay|compare|run (see the header)");
      return 2;
  }
}

if (import.meta.main ?? process.argv[1] === fileURLToPath(import.meta.url)) {
  main().then(
    (code) => process.exit(code),
    (error) => {
      console.error(`typefacts-differential: ${error.message}`);
      process.exit(2);
    }
  );
}
