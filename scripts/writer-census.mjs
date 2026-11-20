#!/usr/bin/env node
// Writer-cause census driver (ADR 1131's M0, measurement only): runs the checker over the frozen corpus project by project, keeps each project's findings in a small
// form, and with `--census` keeps the census the checker prints (`SOLID_CHECKER_WRITER_CENSUS`): every root that may write a built-in's prototype under any name, why,
// and what loads it. With `--whatif-causes` or `--drop-dir` it runs the what-if switch (`SOLID_CHECKER_WHATIF_WRITER_CAUSES`, `SOLID_CHECKER_WHATIF_WRITER_DROP`), which
// leaves roots out of the any-name writers and so moves verdicts: a measurement, never a baseline. Only the what-if build (`make build-what-if`, ADR 0950; its reports are
// tainted and every other consumer refuses them) has the switch: point `--checker` at `rust/target-what-if/release/solid-checker-what-if`.
//
//   node scripts/writer-census.mjs --checker <solid-checker-rust> --typefacts <solid-typefacts> --out <dir>
//     [--census] [--whatif-causes <token,token>] [--drop-dir <dir>] [--projects app/project,...] [--apps a,b] [--jobs 4] [--timeout 900]
//     [--keep-report] [--manifest scripts/corpus/rc21-manifest.json] [--apps-root <frozen corpus apps>] [--runtime-build development]
//
// Per project `<out>/<id>/`: `meta.json` (exit code, wall time, finding counts, the sha256 of the checker's stdout), `rows.json.gz` (every finding as
// `[id, kind, path relative to the apps root, startByte, endByte, analysisContext, blockedBy.kind]`), `census.json` (with `--census`), `report.json.gz` (with
// `--keep-report`: the checker's stdout whole, for a byte comparison). `<id>` is `<app>__<project path with / as _>`. `--drop-dir` names, per project, a file
// `<id>.drop` of roots (one per line) to leave out; a project without one runs with no drop list. Use the release binary and run under sc-heavy.
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

import { DEFAULT_APPS_ROOT } from "./lib/corpus-manifest.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/** The id of one project row, a file name. */
export const projectId = (row) => `${row.app}__${row.project.replace(/[^A-Za-z0-9.-]+/g, "_")}`;

/** A finding as the census keeps it: `[id, kind, path below the apps root, startByte, endByte, analysisContext, blockedBy.kind]`. */
export function reduceFinding(finding, appsRoot) {
  const at = finding.primaryLocation ?? {};
  const path = (at.path ?? "").startsWith(`${appsRoot}/`) ? at.path.slice(appsRoot.length + 1) : at.path ?? "";
  return [finding.id, finding.kind, path, at.startByte ?? null, at.endByte ?? null, finding.analysisContext ?? "", finding.blockedBy?.kind ?? ""];
}

/** Whether a reduced finding is a row of ADR 1114 family 3: an SC2002 obligation under a package that writes a built-in's prototype. */
export const isFamily3 = (reduced) => reduced[1] === "uncertifiable" && reduced[5].includes(":prototype-");

function parseArguments(argv) {
  const options = {
    checker: join(repo, "rust/target/release/solid-checker-rust"),
    typefacts: join(repo, "bin/solid-typefacts"),
    manifest: join(repo, "scripts/corpus/rc21-manifest.json"),
    "apps-root": DEFAULT_APPS_ROOT,
    out: null,
    jobs: "4",
    timeout: "900",
    "runtime-build": null,
    apps: null,
    projects: null,
    "whatif-causes": null,
    "drop-dir": null,
    census: false,
    "keep-report": false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index].replace(/^--/, "");
    if (!(key in options)) throw new Error(`unknown option ${argv[index]}`);
    if (typeof options[key] === "boolean") options[key] = true;
    else {
      if (argv[index + 1] === undefined) throw new Error(`${argv[index]} needs a value`);
      options[key] = argv[(index += 1)];
    }
  }
  if (!options.out) throw new Error("--out <dir> is required");
  return options;
}

/** One checker run of one project, with the census and the what-if switch as `options` say; `{ exitCode, signal, wallMs, stdout, stderr }`. */
export function runProject(options, configuration, row, id) {
  const args = [
    "--format", "json", "--project", join(row.root, row.project),
    "--runtime-target", configuration.runtimeTarget, "--runtime-build", options["runtime-build"] ?? configuration.runtimeBuild, "--rendering", configuration.rendering,
  ];
  const env = { ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: resolve(options.typefacts) };
  for (const name of ["SOLID_CHECKER_WRITER_CENSUS", "SOLID_CHECKER_WHATIF_WRITER_CAUSES", "SOLID_CHECKER_WHATIF_WRITER_DROP"]) delete env[name];
  if (options.census) env.SOLID_CHECKER_WRITER_CENSUS = "1";
  if (options["whatif-causes"]) env.SOLID_CHECKER_WHATIF_WRITER_CAUSES = options["whatif-causes"];
  if (options["drop-dir"] && existsSync(join(options["drop-dir"], `${id}.drop`))) env.SOLID_CHECKER_WHATIF_WRITER_DROP = resolve(join(options["drop-dir"], `${id}.drop`));
  const started = performance.now();
  return new Promise((done) => {
    const child = spawn(resolve(options.checker), args, { cwd: row.root, env });
    const stdout = [];
    const stderr = [];
    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    const timer = setTimeout(() => child.kill("SIGKILL"), Number(options.timeout) * 1000);
    child.on("close", (exitCode, signal) => {
      clearTimeout(timer);
      done({ exitCode: signal ? null : exitCode, signal, wallMs: Math.round(performance.now() - started), stdout: Buffer.concat(stdout), stderr: Buffer.concat(stderr).toString("utf8") });
    });
  });
}

/** Keeps one run in `directory`: `meta.json`, `rows.json.gz`, and per the options `census.json` and `report.json.gz`. */
export function saveResult(directory, options, row, id, result, appsRoot) {
  mkdirSync(directory, { recursive: true });
  let payload = null;
  let parseError = null;
  try {
    payload = JSON.parse(result.stdout.toString("utf8"));
  } catch (error) {
    parseError = `${error.name}: ${error.message}`;
  }
  const findings = (payload?.findings ?? []).map((finding) => reduceFinding(finding, appsRoot));
  const census = result.stderr
    .split("\n")
    .filter((line) => line.startsWith('{"writerCensus":'))
    .map((line) => JSON.parse(line).writerCensus);
  const count = (predicate) => findings.filter(predicate).length;
  const meta = {
    app: row.app,
    project: row.project,
    exitCode: result.exitCode,
    signal: result.signal,
    wallMs: result.wallMs,
    status: payload?.status ?? null,
    parseError,
    findings: findings.length,
    violations: count((reduced) => reduced[1] === "violation"),
    uncertifiable: count((reduced) => reduced[1] === "uncertifiable"),
    family3: count(isFamily3),
    stdoutSha256: createHash("sha256").update(result.stdout).digest("hex"),
    stdoutBytes: result.stdout.length,
    censusLines: census.length,
    whatIfCauses: options["whatif-causes"] ?? null,
    dropFile: options["drop-dir"] && existsSync(join(options["drop-dir"], `${id}.drop`)) ? `${id}.drop` : null,
    stderrTail: result.exitCode !== 0 || parseError ? result.stderr.replace(/\{"writerCensus":.*\n?/g, "").slice(-1500) : "",
  };
  writeFileSync(join(directory, "meta.json"), `${JSON.stringify(meta, null, 1)}\n`);
  writeFileSync(join(directory, "rows.json.gz"), gzipSync(JSON.stringify(findings)));
  if (census.length > 0) writeFileSync(join(directory, "census.json"), JSON.stringify(census.at(-1)));
  if (options["keep-report"]) writeFileSync(join(directory, "report.json.gz"), gzipSync(result.stdout));
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  const manifest = JSON.parse(readFileSync(resolve(options.manifest), "utf8"));
  const appsRoot = resolve(options["apps-root"]);
  const apps = options.apps ? new Set(options.apps.split(",")) : null;
  const projects = options.projects ? new Set(options.projects.split(",")) : null;
  const rows = manifest.projects
    .filter((row) => (!apps || apps.has(row.app)) && (!projects || projects.has(`${row.app}/${row.project}`) || projects.has(projectId(row))))
    .map((row) => ({ app: row.app, project: row.project, root: join(appsRoot, row.root) }));
  for (const row of rows) if (!existsSync(join(row.root, row.project))) throw new Error(`missing project input: ${join(row.root, row.project)}`);
  const out = resolve(options.out);
  mkdirSync(out, { recursive: true });
  let next = 0;
  let finished = 0;
  const worker = async () => {
    while (next < rows.length) {
      const row = rows[next++];
      const id = projectId(row);
      const result = await runProject(options, manifest.configuration, row, id);
      const directory = join(out, id);
      saveResult(directory, options, row, id, result, appsRoot);
      const meta = JSON.parse(readFileSync(join(directory, "meta.json"), "utf8"));
      finished += 1;
      process.stderr.write(`${finished}/${rows.length} ${id} exit=${meta.exitCode} ${meta.wallMs}ms findings=${meta.findings} family3=${meta.family3}\n`);
    }
  };
  await Promise.all(Array.from({ length: Number(options.jobs) }, worker));
}

if (import.meta.url === `file://${process.argv[1]}`) await main();
