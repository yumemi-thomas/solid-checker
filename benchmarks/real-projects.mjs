#!/usr/bin/env bun
// Interleaved base-against-head edit timing on real Solid 2 projects (ADR 0423).
//
//   bun benchmarks/real-projects.mjs --base DIR --head DIR --root PREPARED
//       [--projects a,b] [--n 5] [--settle 250] [--out FILE.jsonl] [--summary FILE.md]
//       [--enforce-ceilings]
//
// DIR holds solid-checker-rust and solid-typefacts side by side. PREPARED is
// benchmarks/real-projects-prepare.mjs's output. Both arms read and edit one
// copy of each project, because symbol ids hash the project path. Each arm
// gets its own retained daemon (a socket is keyed on the binary). For every
// repetition and edit kind the two arms run in alternating order: write the
// edit, time one full-report CLI call, restore the file, make one untimed call
// so the daemon resyncs, settle. Exit 1 on a gated failure.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { appendFileSync, existsSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

import { descendantRssKiB, parseProcessTable } from "./measure-command.mjs";
import { editText, manifestErrors, markdownTable, summarize } from "./lib/real-projects.mjs";

const argv = process.argv.slice(2);
const option = (name, fallback) => {
  const index = argv.indexOf(`--${name}`);
  return index < 0 ? fallback : argv[index + 1];
};
const sleep = ms => Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);

const here = dirname(new URL(import.meta.url).pathname);
const manifest = JSON.parse(readFileSync(join(here, "real-projects", "manifest.json"), "utf8"));
const errors = manifestErrors(manifest);
if (errors.length) throw new Error(`manifest: ${errors.join("; ")}`);

const armDirs = { base: option("base"), head: option("head") };
const root = option("root");
if (!armDirs.base || !armDirs.head || !root) throw new Error("--base DIR --head DIR --root PREPARED are required");
const n = Number(option("n", "5"));
const settle = Number(option("settle", "250"));
const out = option("out");
const summaryFile = option("summary");
const enforceCeilings = argv.includes("--enforce-ceilings");
const only = option("projects")?.split(",");
const projects = manifest.projects.filter(project => !only || only.includes(project.name));
if (only && projects.length !== only.length) throw new Error(`unknown project in --projects ${only.join(",")}`);

const arms = {};
for (const [arm, dir] of Object.entries(armDirs)) {
  const bin = realpathSync(join(dir, "solid-checker-rust"));
  const producer = realpathSync(join(dir, "solid-typefacts"));
  arms[arm] = { bin, producer };
}
if (arms.base.bin === arms.head.bin) throw new Error("base and head must be different binaries: a daemon socket is keyed on the binary");

function cli(arm, project) {
  const env = {
    ...process.env,
    SOLID_CHECKER_DAEMON: "1",
    SOLID_CHECKER_DAEMON_IDLE_SECS: "3600",
    SOLID_CHECKER_TIMINGS: "1",
    SOLID_TYPEFACTS_BIN: arms[arm].producer,
  };
  const started = process.hrtime.bigint();
  const result = spawnSync(arms[arm].bin, ["--project", project.tsconfigPath, "--format", "json"], {
    cwd: dirname(project.tsconfigPath),
    env,
    encoding: "utf8",
    maxBuffer: 1 << 28,
  });
  const ms = Number(process.hrtime.bigint() - started) / 1e6;
  // 0 clean, 1 findings; anything else is a failed check, not a timing.
  if (result.status !== 0 && result.status !== 1) {
    throw new Error(`${arm} on ${project.name} exited ${result.status ?? result.signal}: ${(result.stderr ?? "").slice(0, 500)}`);
  }
  const retained = (result.stderr ?? "").split("\n").some(line => {
    if (!line.startsWith("{")) return false;
    try { return JSON.parse(line).mode === "retained-daemon"; } catch { return false; }
  });
  const answer = (result.stdout ?? "").split(project.root).join("<root>");
  return { ms, retained, hash: createHash("sha256").update(answer).digest("hex").slice(0, 16), findings: findingCount(result.stdout) };
}

function findingCount(stdout) {
  try { return JSON.parse(stdout).findings?.length; } catch { return undefined; }
}

function daemonPids(arm, project) {
  const table = spawnSync("ps", ["-axo", "pid=,args="], { encoding: "utf8", maxBuffer: 1 << 26 }).stdout ?? "";
  const needle = `${arms[arm].bin} --serve --project ${project.tsconfigPath}`;
  return table.split("\n").filter(line => line.includes(needle)).map(line => Number(line.trim().split(/\s+/)[0]));
}

function daemonRssMiB(arm, project) {
  const rows = parseProcessTable(spawnSync("ps", ["-axo", "pid=,ppid=,rss="], { encoding: "utf8", maxBuffer: 1 << 26 }).stdout ?? "");
  return daemonPids(arm, project).reduce((sum, pid) => sum + descendantRssKiB(pid, rows), 0) / 1024;
}

function stopDaemons(project) {
  for (const arm of Object.keys(arms)) {
    for (const pid of daemonPids(arm, project)) {
      try { process.kill(pid, "SIGTERM"); } catch {}
    }
  }
}

const rows = [];
const peakRss = {};
const record = row => {
  rows.push(row);
  if (out) appendFileSync(out, `${JSON.stringify(row)}\n`);
};

let current;
const restore = () => {
  if (!current) return;
  writeFileSync(current.file, current.original);
  stopDaemons(current);
};
for (const signal of ["SIGINT", "SIGTERM"]) process.on(signal, () => { restore(); process.exit(130); });

try {
  for (const entry of projects) {
    const projectRoot = realpathSync(resolve(root, entry.name));
    const project = { ...entry, root: projectRoot, tsconfigPath: join(projectRoot, entry.tsconfig), file: join(projectRoot, entry.editFile) };
    if (!existsSync(project.tsconfigPath)) throw new Error(`${project.tsconfigPath} is missing; run benchmarks/real-projects-prepare.mjs`);
    project.original = readFileSync(project.file, "utf8");
    current = project;
    stopDaemons(project);
    // Warm-up: each arm starts its daemon on the original text and answers once more retained.
    for (const arm of Object.keys(arms)) {
      cli(arm, project);
      const warm = cli(arm, project);
      console.error(`${project.name} ${arm}: warm ${warm.ms.toFixed(0)} ms, retained ${warm.retained}, findings ${warm.findings}`);
    }
    sleep(1000);
    for (let rep = 0; rep < n; rep++) {
      for (const [k, kind] of project.kinds.entries()) {
        const seq = 1000 + rep * 16 + k;
        const edited = editText(project.original, kind, seq, project.bodyAnchor);
        const order = (rep + k) % 2 ? ["head", "base"] : ["base", "head"];
        for (const arm of order) {
          writeFileSync(project.file, edited);
          sleep(50);
          const timed = cli(arm, project);
          record({ project: project.name, kind, rep, arm, ms: +timed.ms.toFixed(2), hash: timed.hash, retained: timed.retained, findings: timed.findings });
          writeFileSync(project.file, project.original);
          sleep(50);
          cli(arm, project);
          const rss = daemonRssMiB(arm, project);
          const key = `${project.name}:${arm}`;
          peakRss[key] = Math.max(peakRss[key] ?? 0, rss);
          sleep(settle);
        }
      }
    }
    restore();
    current = undefined;
  }
} finally {
  restore();
}

const lines = summarize(rows, { enforceCeilings });
let report = markdownTable(lines);
report += `\nPeak retained daemon RSS (daemon and Type Facts child, sampled after each resync), MiB: ${Object.entries(peakRss).map(([key, mib]) => `${key} ${mib.toFixed(0)}`).join(", ")}\n`;
console.log(report);
if (summaryFile) appendFileSync(summaryFile, `## Real-project edit latency\n\n${report}`);
process.exitCode = lines.some(line => line.fails) ? 1 : 0;
