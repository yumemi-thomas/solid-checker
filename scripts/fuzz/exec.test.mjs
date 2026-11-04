import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { createExecutor } from "./exec.mjs";

/** A stand-in executor: a node script answering the protocol as `behaviour` says. */
function fake(behaviour) {
  const dir = mkdtempSync(join(tmpdir(), "fuzz-exec-fake-"));
  const path = join(dir, "fake-exec");
  writeFileSync(path, `#!/usr/bin/env node
const readline = require("node:readline");
const behaviour = ${JSON.stringify(behaviour)};
const lines = readline.createInterface({ input: process.stdin });
let first = true;
lines.on("line", (line) => {
  const message = JSON.parse(line);
  if (first) { first = false; if (behaviour !== "never-ready") process.stdout.write(JSON.stringify({ op: "ready" }) + "\\n"); return; }
  if (behaviour === "garbage") { process.stdout.write("not json\\n"); return; }
  if (behaviour === "answers") process.stdout.write(JSON.stringify({ seq: message.seq, records: { a: { id: "a" } } }) + "\\n");
});
setInterval(() => {}, 1000);
`);
  chmodSync(path, 0o755);
  return path;
}

// Short deadlines for what a test waits out; generous ones for what it needs to happen. A fake's own start (a Node process) or
// answer is no claim of these tests, and under a loaded machine it can outlast 0.8 s.
const short = { readyMs: 800, prepareMs: 800, runBaseMs: 800, runPerWaveMs: 0, closeMs: 500 };
const starts = { ...short, readyMs: 30_000 };
const options = (behaviour, deadlines = starts) => ({ tsc: "tsc", bin: "bin", concurrency: 1, slotRoot: "/tmp", executable: fake(behaviour), deadlines });

describe("the executor's deadlines", () => {
  it("fails a start that is never ready", async () => {
    await expect(createExecutor(options("never-ready", short))).rejects.toThrow(/not ready within/);
  });

  it("fails a request that gets no answer, and every later one", async () => {
    const executor = await createExecutor(options("silent"));
    await expect(executor.run([{ id: "a" }])).rejects.toThrow(/no answer to run/);
    await expect(executor.run([{ id: "b" }])).rejects.toThrow(/no answer to run/);
    await executor.close();
  });

  it("fails on output that is not the protocol", async () => {
    const executor = await createExecutor(options("garbage", { ...starts, runBaseMs: 30_000 }));
    await expect(executor.run([{ id: "a" }])).rejects.toThrow(/printed/);
    await executor.close();
  });

  it("answers as before when the executor answers", async () => {
    const executor = await createExecutor(options("answers", { ...starts, runBaseMs: 30_000 }));
    expect((await executor.run([{ id: "a" }])).records).toEqual({ a: { id: "a" } });
    await executor.close();
  });
});
