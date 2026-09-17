// A long-lived host for `contract generate` and `contract certify`.
//
// The benchmark used to start one CLI process per probe and per phase: ~800
// processes per corpus run, each paying the runtime's module-graph load and
// JIT warm-up again for about 0.3 s of actual work. This worker imports the two
// CLI functions once and runs one request at a time, reproducing exactly what
// packages/cli/bin/solid-checker.mjs would have produced for the same
// arguments: the same stdout and stderr text, and the same exit status (0, or
// 2 with `solid-checker: <message>` on stderr when the command throws).
//
// Protocol: one JSON object per line on stdin, one per line on stdout.
//   request:  { id, kind: "generate" | "certify", args: [...], env: { NAME: value | null } }
//   response: { id, status, stdout, stderr }
// `env` is applied to process.env for the duration of the request (null
// deletes) and restored afterwards. Only variables the CLI reads at call time
// belong there; the launcher snapshots the native environment on first use.
// The worker's own stdout is the protocol channel, so the CLI's writes to
// process.stdout/process.stderr are captured while a request runs. Nothing is
// shared between requests: every CLI call builds its own resolution session
// and scratch, exactly as a fresh process would.

import { createInterface } from "node:readline";

import { certifyContract } from "../../../packages/cli/scripts/certify-contract.mjs";
import { generatePackageContract } from "../../../packages/cli/scripts/generate-package-contract.mjs";

const protocolWrite = process.stdout.write.bind(process.stdout);
const originalStderrWrite = process.stderr.write.bind(process.stderr);

function applyEnvironment(overrides) {
  const previous = new Map();
  for (const [name, value] of Object.entries(overrides ?? {})) {
    previous.set(name, process.env[name]);
    if (value === null || value === undefined) delete process.env[name];
    else process.env[name] = String(value);
  }
  return () => {
    for (const [name, value] of previous) {
      if (value === undefined) delete process.env[name];
      else process.env[name] = value;
    }
  };
}

async function serve(request) {
  const captured = { stdout: "", stderr: "" };
  process.stdout.write = chunk => {
    captured.stdout += String(chunk);
    return true;
  };
  process.stderr.write = chunk => {
    captured.stderr += String(chunk);
    return true;
  };
  const restoreEnvironment = applyEnvironment(request.env);
  let status = 0;
  try {
    if (request.kind === "generate") {
      await generatePackageContract(request.args);
    } else if (request.kind === "certify") {
      await certifyContract(request.args);
    } else {
      throw new Error(`unknown contract command ${request.kind}`);
    }
  } catch (error) {
    // Byte-for-byte what bin/solid-checker.mjs prints and exits with.
    captured.stderr += `solid-checker: ${error instanceof Error ? error.message : error}\n`;
    status = 2;
  } finally {
    restoreEnvironment();
    process.stdout.write = protocolWrite;
    process.stderr.write = originalStderrWrite;
  }
  return { id: request.id, status, stdout: captured.stdout, stderr: captured.stderr };
}

const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
let chain = Promise.resolve();
lines.on("line", line => {
  if (!line.trim()) return;
  chain = chain.then(async () => {
    let request;
    try {
      request = JSON.parse(line);
    } catch (error) {
      protocolWrite(`${JSON.stringify({ id: null, status: 2, stdout: "", stderr: `solid-checker: malformed worker request: ${error.message}\n` })}\n`);
      return;
    }
    const response = await serve(request);
    protocolWrite(`${JSON.stringify(response)}\n`);
  });
});
// Stdin EOF is this worker's only signal that nobody is listening any more. It
// arrives on an orderly recycle (the pool calls `stdin.end()` on an idle
// worker) and, just as reliably, when the runner *dies* -- the kernel closes
// the write end with it, whether it exited, threw, or was killed outright.
//
// Both cases end the same way, and they must. Waiting for `chain` to settle
// first looks polite and is the leak: a certification runs for minutes, nobody
// will ever read its answer, and meanwhile this worker's native children keep
// a machine busy. A crashed corpus run left eight of these alive for half an
// hour, two of their children pinned at over 200% CPU, and the run that
// replaced it was starved by them -- which read, from outside, as a hang.
//
// So exit now rather than when the work happens to finish. When the pool told
// us we lead our own process group, signal the *group*: that takes down the
// native checker children this worker started, which `process.exit` cannot
// reach and which are the part that actually burns CPU. It kills this process
// too, which is the intent -- the line below is only reached when the group
// kill is unavailable or refused.
lines.on("close", () => {
  if (process.env.SOLID_CHECKER_CLI_WORKER_GROUP_LEADER === "1") {
    try {
      process.kill(-process.pid, "SIGKILL");
    } catch {
      // Already gone, or not ours to signal: fall through and exit alone.
    }
  }
  process.exit(0);
});
