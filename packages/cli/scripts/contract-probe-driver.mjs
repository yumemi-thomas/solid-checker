// Runtime-probe process driver for stable main schema version 1. It transports
// Rust-issued, independently versioned probe sessions into fresh workers and
// returns raw runs. It never interprets an event, a missing marker, or a
// semantic claim.

import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const worker = fileURLToPath(new URL("./contract-probe-worker.mjs", import.meta.url));

export function sha256Bytes(bytes) {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

function timeoutRun(session) {
  return {
    session: session.id,
    environment: session.mode.environment,
    isolation: {
      process: `timeout:${session.id}`,
      realm: `timeout-realm:${session.id}`,
      moduleInstance: `timeout-module:${session.id}`
    },
    drainedMicrotasks: 0,
    drainedMacrotasks: 0,
    outcome: { kind: "timeout" }
  };
}

async function runSession(session, baseDirectory) {
  const module = resolve(baseDirectory, session.module);
  const observed = sha256Bytes(readFileSync(module));
  if (observed !== session.construction) {
    return {
      session: session.id,
      environment: session.mode.environment,
      isolation: {
        process: `refused:${session.id}`,
        realm: `refused-realm:${session.id}`,
        moduleInstance: `refused-module:${session.id}`
      },
      drainedMicrotasks: 0,
      drainedMacrotasks: 0,
      outcome: {
        kind: "refused",
        reason: `recipe module digest ${observed} does not match planned construction ${session.construction}`
      }
    };
  }
  return await new Promise((resolveRun, reject) => {
    const child = spawn(process.execPath, [worker], {
      cwd: baseDirectory,
      env: { ...process.env, SOLID_CHECKER_PROBE_RECIPE: module },
      stdio: ["pipe", "pipe", "pipe"]
    });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", chunk => (stdout += chunk));
    child.stderr.on("data", chunk => (stderr += chunk));
    child.on("error", reject);
    const timer = setTimeout(() => child.kill("SIGKILL"), session.policy.timeoutMillis);
    child.on("close", (status, signal) => {
      clearTimeout(timer);
      if (signal === "SIGKILL") return resolveRun(timeoutRun(session));
      if (status !== 0) {
        return reject(new Error(stderr.trim() || `runtime probe worker exited ${status}`));
      }
      try {
        resolveRun(JSON.parse(stdout));
      } catch (error) {
        reject(new Error(`runtime probe worker returned invalid JSON: ${error.message}`));
      }
    });
    child.stdin.end(`${JSON.stringify(session)}\n`);
  });
}

export async function runProbeSessions(plan, requestPath) {
  if (
    plan?.format !== "solid-checker-runtime-probe-plan" ||
    plan?.schemaVersion !== 2 ||
    !Array.isArray(plan.sessions)
  ) {
    throw new TypeError("native runtime probe plan must use runtime-probe schema version 2");
  }
  const baseDirectory = dirname(resolve(requestPath));
  const runs = [];
  for (const session of plan.sessions) runs.push(await runSession(session, baseDirectory));
  return runs;
}
