// Runtime-probe process driver for the audit path of stable main schema version 1.
// It transports Rust-issued, independently versioned probe sessions into fresh
// workers and returns raw runs. It never interprets an event, a missing
// marker, or a semantic claim.
//
// This driver carries no authority. Inside a certification transaction Rust
// launches the worker itself
// (`contract_certification::probe_harness::run_probe_gates`), hashes the Node
// executable and the harness image against digests compiled into the verifier,
// and re-derives the recipe construction digest from the bytes it copied. The
// construction check below therefore remains a useful early failure for audit
// runs and phase measurements, but it is no longer what enforces recipe
// identity.
//
// Concretely, this path establishes **no realm integrity**: it verifies no Node
// executable digest, recomputes no harness source manifest, runs against no
// private snapshot copy, and takes no watched isolation census. It does mirror
// the certification path's environment allowlist when launching a worker
// (below), because an inherited `NODE_OPTIONS` would run a module of the
// caller's choosing before the worker captures its primordials — but that is
// hygiene for a measurement, not authority.

import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { tmpdir } from "node:os";
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

function refusedRun(session, reason) {
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
    outcome: { kind: "refused", reason }
  };
}

async function runSession(session, baseDirectory) {
  const module = resolve(baseDirectory, session.module);
  const observed = sha256Bytes(readFileSync(module));
  if (observed !== session.construction) {
    return refusedRun(
      session,
      `recipe module digest ${observed} does not match planned construction ${session.construction}`
    );
  }
  return await new Promise((resolveRun, reject) => {
    // Descriptor 3 carries both frames. The worker writes nothing to stdout on
    // purpose: package top-level code runs in its realm and can replace
    // `process.stdout.write`, so a stream that stream can reach must not be
    // the one a frame is read from. Stdout is still piped and drained so a
    // package writing there cannot fill a pipe buffer and stall the worker.
    // The certification path's environment allowlist, mirrored rather than
    // inherited. Inheriting `process.env` handed the worker `NODE_OPTIONS`,
    // and `NODE_OPTIONS=--import …` or `--require …` runs a module of the
    // caller's choosing *before* the worker evaluates — which is before the
    // primordials it reports with are captured and before the intrinsic
    // prototypes are frozen. No `PATH`: the worker is launched by absolute
    // path and must not find anything by name.
    //
    // What this does **not** buy: the audit path still establishes no realm
    // integrity. It hashes no Node executable, recomputes no harness manifest,
    // copies no snapshot, and takes no watched census, so nothing here is
    // authority-bearing (see the header). Mirroring the allowlist only keeps
    // the *measurement* from being shaped by the caller's shell.
    const child = spawn(process.execPath, [worker], {
      cwd: baseDirectory,
      env: {
        HOME: baseDirectory,
        // The one deliberate deviation from the certification allowlist, which
        // points `TMPDIR` at its private directory: the audit path's base
        // directory is an ordinary checked-out tree (`scripts/`, for the phase
        // 16 measurement), and pointing `TMPDIR` there would let an incidental
        // temporary file land in the worktree.
        TMPDIR: tmpdir(),
        LANG: "C",
        LC_ALL: "C",
        NODE_OPTIONS: ""
      },
      stdio: ["pipe", "pipe", "pipe", "pipe"]
    });
    let report = "";
    let stderr = "";
    child.stdio[3].setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdio[3].on("data", chunk => (report += chunk));
    child.stdout.on("data", () => {});
    child.stderr.on("data", chunk => (stderr += chunk));
    child.on("error", reject);
    const timer = setTimeout(() => child.kill("SIGKILL"), session.policy.timeoutMillis);
    child.on("close", (status, signal) => {
      clearTimeout(timer);
      if (signal === "SIGKILL") return resolveRun(timeoutRun(session));
      if (status !== 0) {
        return reject(new Error(stderr.trim() || `runtime probe worker exited ${status}`));
      }
      // Exactly the startup frame and one run frame. Taking the last line
      // instead would let a worker that wrote several choose which one this
      // layer believes.
      const lines = report.split("\n").filter(line => line.trim().length > 0);
      if (lines.length !== 2) {
        return reject(
          new Error(
            `runtime probe worker wrote ${lines.length} report frames; exactly a startup frame and one run frame are the protocol`
          )
        );
      }
      let startup;
      let run;
      try {
        startup = JSON.parse(lines[0]);
        run = JSON.parse(lines[1]);
      } catch (error) {
        return reject(new Error(`runtime probe worker returned invalid JSON: ${error.message}`));
      }
      if (
        startup?.format !== "solid-checker-probe-worker-startup" ||
        startup.protocol !== session.mode.environment.runtime.protocol
      ) {
        return reject(new Error("runtime probe worker startup frame does not match the session"));
      }
      // The worker echoes the session's declared environment verbatim — it
      // asserts nothing about itself — so this layer is where an audit request
      // that declares a runtime it is not actually running has to be caught.
      if (startup.nodeVersion !== session.mode.environment.runtime.version) {
        return resolveRun(
          refusedRun(
            session,
            `worker runs Node ${startup.nodeVersion}, but the request declares ${session.mode.environment.runtime.version}`
          )
        );
      }
      resolveRun(run);
    });
    // Protocol v7: the recipe module travels in the session frame rather than
    // the environment, so the certification path can boot a worker before its
    // session is chosen. Both launchers must agree on the frame.
    child.stdin.end(`${JSON.stringify({ ...session, recipe: module })}\n`);
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
