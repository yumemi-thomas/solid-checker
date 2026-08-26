#!/usr/bin/env bun

import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { mkdirSync } from "node:fs";

const POLL_MS = 10;

export function parseProcessTable(text) {
  const rows = [];
  for (const line of String(text).split("\n")) {
    const match = /^\s*(\d+)\s+(\d+)\s+(\d+)\s*$/.exec(line);
    if (!match) continue;
    rows.push({ pid: Number(match[1]), ppid: Number(match[2]), rssKiB: Number(match[3]) });
  }
  return rows;
}

export function descendantRssKiB(rootPid, rows) {
  const descendants = new Set([rootPid]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const row of rows) {
      if (!descendants.has(row.pid) && descendants.has(row.ppid)) {
        descendants.add(row.pid);
        changed = true;
      }
    }
  }
  return rows
    .filter(row => descendants.has(row.pid))
    .reduce((sum, row) => sum + row.rssKiB, 0);
}

function usage() {
  return "Usage: bun benchmarks/measure-command.mjs --json <FILE> -- <COMMAND> [ARGS...]";
}

function parseArgs(argv) {
  const separator = argv.indexOf("--");
  const jsonIndex = argv.indexOf("--json");
  if (separator === -1 || jsonIndex === -1 || jsonIndex + 1 >= separator) {
    throw new Error(usage());
  }
  const command = argv.slice(separator + 1);
  if (!command.length) throw new Error(usage());
  return { json: resolve(argv[jsonIndex + 1]), command };
}

async function processTable() {
  const child = Bun.spawn(["/bin/ps", "-axo", "pid=,ppid=,rss="], {
    stdout: "pipe",
    stderr: "pipe"
  });
  const [stdout, stderr, status] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited
  ]);
  if (status !== 0) throw new Error(stderr.trim() || `ps exited ${status}`);
  return parseProcessTable(stdout);
}

export async function measureCommand({ json, command, pollMs = POLL_MS }) {
  const startedAt = new Date().toISOString();
  const start = performance.now();
  const child = spawn(command[0], command.slice(1), {
    env: process.env,
    stdio: "inherit"
  });
  let sampleCount = 0;
  let maxProcessTreeRssKiB = 0;
  let samplingError = null;
  let sampling = false;
  const sample = async () => {
    if (sampling || child.exitCode !== null || child.signalCode !== null) return;
    sampling = true;
    try {
      const rows = await processTable();
      maxProcessTreeRssKiB = Math.max(maxProcessTreeRssKiB, descendantRssKiB(child.pid, rows));
      sampleCount += 1;
    } catch (error) {
      samplingError ??= String(error?.message ?? error);
    } finally {
      sampling = false;
    }
  };
  await sample();
  const timer = setInterval(sample, pollMs);
  const result = await new Promise(resolvePromise => {
    child.on("error", error => resolvePromise({ code: null, signal: null, spawnError: String(error.message) }));
    child.on("close", (code, signal) => resolvePromise({ code, signal, spawnError: null }));
  });
  clearInterval(timer);
  while (sampling) await Bun.sleep(1);
  const finishedAt = new Date().toISOString();
  const report = {
    schemaVersion: 1,
    command,
    startedAt,
    finishedAt,
    elapsedMs: Math.round((performance.now() - start) * 1000) / 1000,
    exitCode: result.code,
    signal: result.signal,
    spawnError: result.spawnError,
    memory: {
      method: "sum of resident-set KiB for the sampled command process tree",
      pollIntervalMs: pollMs,
      samples: sampleCount,
      maxProcessTreeRssKiB: sampleCount ? maxProcessTreeRssKiB : null,
      samplingError
    }
  };
  mkdirSync(dirname(json), { recursive: true });
  writeFileSync(json, `${JSON.stringify(report, null, 2)}\n`);
  return report;
}

const isMain = import.meta.main;
if (isMain) {
  try {
    const options = parseArgs(process.argv.slice(2));
    const report = await measureCommand(options);
    process.exitCode = report.exitCode ?? (report.signal || report.spawnError ? 1 : 0);
  } catch (error) {
    console.error(`measure-command: ${error.message}`);
    process.exitCode = 2;
  }
}
