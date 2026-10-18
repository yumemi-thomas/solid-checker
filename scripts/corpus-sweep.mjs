#!/usr/bin/env node
// Corpus sweep: run the checker over every project in a corpus manifest and
// write one JSON report (findings per project, binary digests, configuration).
//
//   node scripts/corpus-sweep.mjs [--manifest scripts/corpus/rc13-manifest.json]
//     [--apps-root ../solid-checker-rc13-corpus/apps]
//     [--checker rust/target/release/solid-checker-rust]
//     [--typefacts bin/solid-typefacts] [--jobs 4] --out <report.json>
//
// Use the release binary: a debug build inflates certification about 19x.
// Prefix the run with ~/.local/bin/sc-heavy when other agents share the
// machine. Compare two reports with scripts/corpus-sweep-compare.mjs, by
// finding key rather than by count.
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const options = {
  manifest: join(repo, "scripts/corpus/rc13-manifest.json"),
  "apps-root": resolve(repo, "../solid-checker-rc13-corpus/apps"),
  checker: join(repo, "rust/target/release/solid-checker-rust"),
  typefacts: join(repo, "bin/solid-typefacts"),
  jobs: "4",
  timeout: "900",
  out: null,
};
const argv = process.argv.slice(2);
for (let index = 0; index < argv.length; index += 2) {
  const key = argv[index].replace(/^--/, "");
  if (!(key in options) || argv[index + 1] === undefined) {
    throw new Error(`unknown or incomplete option ${argv[index]}`);
  }
  options[key] = argv[index + 1];
}
if (!options.out) throw new Error("--out <report.json> is required");

// A relative producer path fails every project (the checker runs with each
// project root as its working directory), so every path is made absolute.
const manifestPath = resolve(options.manifest);
const appsRoot = resolve(options["apps-root"]);
const checker = resolve(options.checker);
const typefacts = resolve(options.typefacts);
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
const configuration = manifest.configuration;
const rows = manifest.projects.map((row) => ({
  app: row.app,
  root: join(appsRoot, row.root),
  project: row.project,
}));
for (const row of rows) {
  const project = join(row.root, row.project);
  if (!existsSync(project)) throw new Error(`missing project input: ${project}`);
}

const env = { ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: typefacts };
const timeoutMs = Number(options.timeout) * 1000;

function run(row) {
  const args = [
    "--format", "json",
    "--project", join(row.root, row.project),
    "--runtime-target", configuration.runtimeTarget,
    "--runtime-build", configuration.runtimeBuild,
    "--rendering", configuration.rendering,
  ];
  const started = performance.now();
  return new Promise((done) => {
    const child = spawn(checker, args, { cwd: row.root, env });
    const stdout = [];
    const stderr = [];
    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    const timer = setTimeout(() => child.kill("SIGKILL"), timeoutMs);
    child.on("close", (exitCode, signal) => {
      clearTimeout(timer);
      const wallMs = Math.round(performance.now() - started);
      const text = Buffer.concat(stdout).toString("utf8");
      const errorText = Buffer.concat(stderr).toString("utf8");
      let payload = null;
      let parseError = null;
      try {
        payload = JSON.parse(text);
      } catch (error) {
        parseError = `${error.name}: ${error.message}`;
      }
      done({
        ...row,
        exitCode: signal ? null : exitCode,
        signal,
        wallMs,
        analysisStatus: payload?.status ?? null,
        findings: payload?.findings ?? [],
        metrics: payload?.metrics ?? null,
        parseError,
        stderr: exitCode !== 0 || parseError ? errorText.slice(-4000) : "",
      });
    });
  });
}

const results = [];
let next = 0;
async function worker() {
  while (next < rows.length) {
    const row = rows[next++];
    const result = await run(row);
    results.push(result);
    process.stderr.write(
      `${results.length}/${rows.length} ${result.app} ${result.project} exit=${result.exitCode} ${result.wallMs}ms\n`,
    );
  }
}
await Promise.all(Array.from({ length: Number(options.jobs) }, worker));

results.sort(
  (left, right) =>
    left.app.localeCompare(right.app) ||
    left.project.localeCompare(right.project) ||
    left.root.localeCompare(right.root),
);
const sha256 = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");
const kinds = {};
for (const result of results) {
  for (const finding of result.findings) kinds[finding.kind] = (kinds[finding.kind] ?? 0) + 1;
}
const summary = {
  checker,
  checkerSha256: sha256(checker),
  producer: typefacts,
  producerSha256: sha256(typefacts),
  manifestSha256: sha256(manifestPath),
  configuration: { ...configuration, daemon: false },
  projectCount: results.length,
  findingCount: results.reduce((total, result) => total + result.findings.length, 0),
  findingKinds: kinds,
  exitErrors: results.filter((result) => result.exitCode !== 0).length,
  parseErrors: results.filter((result) => result.parseError !== null).length,
  results,
};
writeFileSync(resolve(options.out), `${JSON.stringify(summary, null, 2)}\n`);
const { results: _, ...head } = summary;
console.log(JSON.stringify(head, null, 2));
