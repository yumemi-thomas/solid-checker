#!/usr/bin/env bun
// Measure the retained actor after one materialized analysis. On macOS,
// --vmmap-all reports the reclaimability-aware physical footprint; pair it
// with --max-physical-mib=<MiB> for a regression gate.

import { mkdtempSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const repository = resolve(import.meta.dirname, "..");
const checker = resolve(
  process.env.SOLID_CHECKER_BIN ??
    join(repository, "rust/target/release/solid-checker-rust")
);
const typefacts = resolve(
  process.env.SOLID_TYPEFACTS_BIN ?? join(repository, "bin/solid-typefacts")
);
const project = resolve(
  process.env.SOLID_CHECKER_MEMORY_PROJECT ??
    process.argv.find(argument => argument.startsWith("--project="))?.slice(10) ??
    join(repository, "fixtures/engine/eslint-reactivity-v2/tsconfig.json")
);
const maximum = Number(
  process.argv.find(argument => argument.startsWith("--max-total-mib="))?.slice(16) ??
    "Infinity"
);
const physicalMaximum = Number(
  process.argv
    .find(argument => argument.startsWith("--max-physical-mib="))
    ?.slice(19) ?? "Infinity"
);
const vmmapAll = process.argv.includes("--vmmap-all");
if (physicalMaximum !== Infinity && !vmmapAll) {
  throw new Error("--max-physical-mib requires --vmmap-all");
}
const idleSeconds =
  process.argv.find(argument => argument.startsWith("--idle-secs="))?.slice(12) ??
  "10";
const temporary = mkdtempSync(join(tmpdir(), "solid-checker-memory-"));
const producer = join(temporary, "solid-typefacts");

function run(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    ...options
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      [result.stderr, result.stdout].filter(Boolean).join("\n").trim() ||
        `${command} exited ${result.status}`
    );
  }
  return result;
}

function physicalFootprintMib(summary) {
  const match = summary.match(/^Physical footprint:\s+([\d.]+)([KMG])\b/m);
  if (!match) return null;
  const value = Number(match[1]);
  if (match[2] === "K") return value / 1024;
  if (match[2] === "G") return value * 1024;
  return value;
}

try {
  symlinkSync(typefacts, producer);
  const environment = {
    ...process.env,
    SOLID_CHECKER_DAEMON: "1",
    SOLID_CHECKER_DAEMON_IDLE_SECS: idleSeconds,
    SOLID_CHECKER_DAEMON_MAX_RSS_MB: "0"
  };
  delete environment.GOMEMLIMIT;
  const started = process.hrtime.bigint();
  const check = run(
    checker,
    ["--project", project, "--typefacts", producer, "--format", "json"],
    { env: environment }
  );
  const elapsedNs = Number(process.hrtime.bigint() - started);
  const rows = run("ps", ["-axo", "pid=,ppid=,rss=,command="]).stdout
    .trim()
    .split("\n")
    .flatMap(line => {
      const match = line.trim().match(/^(\d+)\s+(\d+)\s+(\d+)\s+(.*)$/);
      return match
        ? [{
            pid: Number(match[1]),
            parent: Number(match[2]),
            residentKib: Number(match[3]),
            command: match[4]
          }]
        : [];
    });
  const root = rows
    .filter(
      row =>
        row.command.includes(`${basename(checker)} --serve`) &&
        row.command.includes(project)
    )
    .sort((left, right) => right.pid - left.pid)[0];
  if (!root) throw new Error("retained checker process was not found");
  const included = new Set([root.pid]);
  for (;;) {
    let changed = false;
    for (const row of rows) {
      if (!included.has(row.pid) && included.has(row.parent)) {
        included.add(row.pid);
        changed = true;
      }
    }
    if (!changed) break;
  }
  const processes = rows.filter(row => included.has(row.pid));
  const totalMib =
    processes.reduce((total, process) => total + process.residentKib, 0) / 1024;
  const physicalFootprints = vmmapAll
    ? Object.fromEntries(
        processes.map(process => {
          const summary = run("/usr/bin/vmmap", [
            "-summary",
            String(process.pid)
          ]).stdout;
          return [
            process.pid,
            {
              command: process.command,
              physicalMib: physicalFootprintMib(summary),
              summary
            }
          ];
        })
      )
    : undefined;
  const physicalTotalMib = physicalFootprints
    ? Object.values(physicalFootprints).reduce(
        (total, process) => total + (process.physicalMib ?? 0),
        0
      )
    : undefined;
  const report = {
    schemaVersion: 1,
    project,
    elapsedNs,
    responseBytes: Buffer.byteLength(check.stdout),
    totalMib,
    processes: processes.map(process => ({
      pid: process.pid,
      role: process.pid === root.pid ? "checker" : "descendant",
      residentMib: process.residentKib / 1024,
      command: process.command
    })),
    ...(process.argv.includes("--vmmap-summary")
      ? { vmmapSummary: run("/usr/bin/vmmap", ["-summary", String(root.pid)]).stdout }
      : {}),
    ...(physicalFootprints ? { physicalTotalMib, physicalFootprints } : {}),
    ...(process.argv.includes("--heap-summary")
      ? { heapSummary: run("/usr/bin/heap", ["-s", String(root.pid)]).stdout }
      : {})
  };
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  if (totalMib > maximum) {
    throw new Error(
      `retained process tree uses ${totalMib.toFixed(1)} MiB; expected at most ${maximum} MiB`
    );
  }
  if (physicalTotalMib !== undefined && physicalTotalMib > physicalMaximum) {
    throw new Error(
      `retained process tree has a ${physicalTotalMib.toFixed(1)} MiB physical footprint; expected at most ${physicalMaximum} MiB`
    );
  }
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
