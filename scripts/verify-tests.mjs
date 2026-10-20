import { spawn } from "node:child_process";
import { closeSync, mkdirSync, mkdtempSync, openSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

export function verificationJobs(runner, env = process.env) {
  const cargo = ["cargo", "+1.97"];
  const common = ["--manifest-path", "rust/Cargo.toml", "--workspace"];
  const profile = env.SOLID_CHECKER_CARGO_PROFILE || "verify";
  return [
    { name: "go-test-race", commands: [["go", "test", "-race", "./apps/solid-typefacts/..."]],
      env: { ...env, GOMAXPROCS: env.GOMAXPROCS || "4" } },
    { name: "test-workspace", commands: runner === "nextest" ? [
      [...cargo, "nextest", "run", "--config-file", "scripts/nextest.toml", "--profile", "verify", "--cargo-profile", profile, ...common],
      [...cargo, "test", "--profile", profile, ...common, "--doc"],
    ] : [[...cargo, "test", "--profile", profile, ...common]],
    env: { ...env, RUST_TEST_THREADS: env.RUST_TEST_THREADS || "8" } },
  ];
}

// Each command owns a process group so interruption also stops compiler and
// fixture subprocesses. All jobs settle before the caller can run benchmarks.
export async function runTestJobs(jobs, { directory, parallel = true } = {}) {
  mkdirSync(directory, { recursive: true });
  const logDirectory = mkdtempSync(resolve(directory, "run-"));
  console.log(`Verification test logs: ${logDirectory}`);
  const active = new Set();
  let interrupted = null;
  let killTimer;
  const kill = signal => {
    for (const pid of active) {
      try { process.kill(-pid, signal); }
      catch (error) { if (error.code !== "ESRCH") throw error; }
    }
  };
  const onSignal = signal => {
    if (interrupted) return;
    interrupted = signal;
    kill("SIGTERM");
    killTimer = setTimeout(() => kill("SIGKILL"), 1000);
  };
  const signals = ["SIGINT", "SIGTERM", "SIGHUP"];
  const handlers = signals.map(signal => () => onSignal(signal));
  signals.forEach((signal, i) => process.on(signal, handlers[i]));
  const started = performance.now();
  const run = async job => {
    const start = performance.now();
    const log = resolve(logDirectory, `${job.name}.log`);
    let status = null;
    let signal = null;
    const fd = openSync(log, "w");
    try {
      for (const command of job.commands) {
        if (interrupted) break;
        const result = await new Promise(resolveResult => {
          const child = spawn(command[0], command.slice(1), {
            env: job.env || process.env, detached: true, stdio: ["ignore", fd, fd],
          });
          if (child.pid) active.add(child.pid);
          child.on("error", error => {
            writeFileSync(fd, `${error.stack}\n`);
            resolveResult({ status: 127, signal: null });
          });
          child.on("close", (code, childSignal) => {
            // On interruption retain the group until the escalation has run:
            // the direct child may exit before its grandchildren do.
            if (!interrupted) active.delete(child.pid);
            resolveResult({ status: code, signal: childSignal });
          });
        });
        ({ status, signal } = result);
        if (status !== 0 || signal) break;
      }
    } finally { closeSync(fd); }
    const result = { name: job.name, status, signal, seconds: (performance.now() - start) / 1000, log };
    console.log(`${job.name}: ${status === 0 && !interrupted ? "PASS" : "FAIL"} (${result.seconds.toFixed(2)}s); ${log}`);
    return result;
  };
  let results;
  try {
    if (parallel) results = await Promise.all(jobs.map(run));
    else {
      results = [];
      for (const job of jobs) results.push(await run(job));
    }
    if (interrupted) {
      // Do not leave descendants behind even if their parent exited on TERM.
      await new Promise(resolveWait => setTimeout(resolveWait, 1100));
      kill("SIGKILL");
    }
    const summary = { seconds: (performance.now() - started) / 1000, interrupted, results };
    writeFileSync(resolve(logDirectory, "summary.json"), `${JSON.stringify(summary, null, 2)}\n`);
    return summary;
  } finally {
    clearTimeout(killTimer);
    kill("SIGKILL");
    signals.forEach((signal, i) => process.off(signal, handlers[i]));
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const runner = process.argv[2];
  const parallel = process.env.SOLID_CHECKER_VERIFY_PARALLEL ?? "1";
  if (!["test", "nextest"].includes(runner) || !["0", "1"].includes(parallel)) {
    throw new Error("Expected runner test/nextest and SOLID_CHECKER_VERIFY_PARALLEL=0/1");
  }
  const summary = await runTestJobs(verificationJobs(runner), {
    directory: "rust/target/verify-logs", parallel: parallel === "1",
  });
  process.exitCode = summary.interrupted ? 128 + ({ SIGINT: 2, SIGTERM: 15, SIGHUP: 1 }[summary.interrupted])
    : summary.results.some(result => result.status !== 0 || result.signal) ? 1 : 0;
}
