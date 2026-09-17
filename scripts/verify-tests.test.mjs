import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { test } from "vitest";
import { runTestJobs, verificationJobs } from "./verify-tests.mjs";

const node = execFileSync("node", ["-p", "process.execPath"], { encoding: "utf8" }).trim();
const job = (name, code) => ({ name, commands: [[node, "-e", code]] });
const temporary = async fn => {
  const directory = mkdtempSync(join(tmpdir(), "verify-jobs-"));
  try { await fn(directory); } finally { rmSync(directory, { recursive: true, force: true }); }
};

test("parallel jobs actually overlap and preserve both failures and logs", async () => temporary(async directory => {
  const jobs = ["a", "b"].map((name, i) => job(name, `
    const fs = require('node:fs');
    fs.writeFileSync(${JSON.stringify(join(directory, name))}, 'ready');
    const timer = setInterval(() => {
      if (fs.existsSync(${JSON.stringify(join(directory, i ? "a" : "b"))})) {
        clearInterval(timer); console.log('${name} failure'); process.exit(${i + 2});
      }
    }, 10);
    setTimeout(() => process.exit(99), 2000);
  `));
  const result = await runTestJobs(jobs, { directory });
  assert.deepEqual(result.results.map(row => row.status), [2, 3]);
  for (const row of result.results) assert.match(readFileSync(row.log, "utf8"), /failure/);
}));

test("sequential mode waits and spawn failures cannot pass", async () => temporary(async directory => {
  const marker = join(directory, "done");
  const result = await runTestJobs([
    job("first", `setTimeout(() => require('node:fs').writeFileSync(${JSON.stringify(marker)}, ''), 50)`),
    job("second", `process.exit(require('node:fs').existsSync(${JSON.stringify(marker)}) ? 0 : 7)`),
    { name: "missing", commands: [[join(directory, "missing-executable")]] },
  ], { directory, parallel: false });
  assert.deepEqual(result.results.map(row => row.status), [0, 0, 127]);
}));

test("a failed command does not run the next command in its suite", async () => temporary(async directory => {
  const marker = join(directory, "unexpected");
  const suite = job("suite", "process.exit(5)");
  suite.commands.push(job("unused", `require('node:fs').writeFileSync(${JSON.stringify(marker)}, '')`).commands[0]);
  const result = await runTestJobs([suite], { directory });
  assert.equal(result.results[0].status, 5);
  assert.equal(existsSync(marker), false);
}));

test("nextest still runs doctests sequentially with inherited certification pins", () => {
  const env = { SOLID_CHECKER_EXPECT_PROBE_PINS: "1", GOMAXPROCS: "2", RUST_TEST_THREADS: "3" };
  const [go, rust] = verificationJobs("nextest", env);
  assert.equal(go.env.GOMAXPROCS, "2");
  assert.equal(rust.env.RUST_TEST_THREADS, "3");
  assert.equal(rust.env.SOLID_CHECKER_EXPECT_PROBE_PINS, "1");
  assert.equal(rust.commands.length, 2);
  assert.equal(rust.commands[1].at(-1), "--doc");
  assert.equal(verificationJobs("test")[1].commands.length, 1);
});

test("interrupting the runner kills even a TERM-resistant grandchild", async () => temporary(async directory => {
  const pidFile = join(directory, "grandchild");
  const grandchild = `require('node:fs').writeFileSync(${JSON.stringify(pidFile)}, String(process.pid)); process.on('SIGTERM', () => {}); setInterval(() => {}, 100);`;
  const parent = `require('node:child_process').spawn(process.execPath, ['-e', ${JSON.stringify(grandchild)}], {stdio:'inherit'}); setInterval(() => {}, 100);`;
  const moduleUrl = pathToFileURL(join(process.cwd(), "scripts/verify-tests.mjs")).href;
  const code = `import {runTestJobs} from ${JSON.stringify(moduleUrl)}; const result = await runTestJobs(${JSON.stringify([job("tree", parent)])}, {directory:${JSON.stringify(directory)}}); process.exitCode = result.interrupted ? 130 : 0;`;
  const child = spawn(node, ["--input-type=module", "-e", code], { stdio: "ignore" });
  const exited = new Promise(resolve => child.on("exit", resolve));
  let grandchildPid;
  try {
    const deadline = Date.now() + 4000;
    while (!existsSync(pidFile) && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 20));
    assert.ok(existsSync(pidFile));
    grandchildPid = Number(readFileSync(pidFile, "utf8"));
    child.kill("SIGTERM");
    assert.equal(await exited, 130);
    const stoppedBy = Date.now() + 2000;
    while (Date.now() < stoppedBy) {
      try { process.kill(grandchildPid, 0); } catch (error) { assert.equal(error.code, "ESRCH"); return; }
      await new Promise(resolve => setTimeout(resolve, 20));
    }
    assert.fail("grandchild survived runner interruption");
  } finally {
    child.kill("SIGKILL");
    if (grandchildPid) { try { process.kill(grandchildPid, "SIGKILL"); } catch {} }
  }
}));
