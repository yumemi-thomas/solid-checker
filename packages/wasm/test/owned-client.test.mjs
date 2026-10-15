// The owned client's settlement, against a scripted worker: a malformed
// result must settle the active check with the queued and later ones, and stop
// the worker. The packaged worker path is in `make wasm-parity`.
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { test } from "vitest";
import { attach } from "../owned-client.mjs";

class ScriptedWorker extends EventEmitter {
  constructor(reply) {
    super();
    this.reply = reply;
    this.checks = 0;
    this.terminated = 0;
  }
  postMessage(message) {
    queueMicrotask(() => {
      if (message.type === "init") this.emit("message", { type: "ready" });
      else {
        this.checks++;
        this.emit("message", this.reply(message));
      }
    });
  }
  terminate() {
    this.terminated++;
    queueMicrotask(() => this.emit("exit", 1));
    return Promise.resolve(1);
  }
  ref() {}
  unref() {}
}

const init = { checker: new ArrayBuffer(0), producer: new ArrayBuffer(0), mount: { root: "/m", entries: [] }, transfer: [] };
const outcome = promise => {
  const state = { settled: "pending" };
  promise.then(value => Object.assign(state, { settled: "resolved", value }), error => Object.assign(state, { settled: "rejected", error }));
  return state;
};
const flush = () => new Promise(resolve => setImmediate(resolve));

for (const [name, forge, message] of [
  ["invalid JSON", ({ id }) => ({ type: "result", id, status: 0, output: "{", transport: {} }), /^the owned checker answered malformed output: /],
  ["an answer of the wrong shape", ({ id }) => ({ type: "result", id, status: 0, output: '{"error":"forged"}', transport: {} }),
    /^the owned checker answered malformed output: status 0 with an unexpected answer$/],
  ["an unexpected status", ({ id }) => ({ type: "result", id, status: 7, output: '{"snapshot":{},"notes":[]}', transport: {} }),
    /^the owned checker answered malformed output: status 7 with an unexpected answer$/],
]) {
  test(`${name} rejects the active, queued and later checks and stops the worker`, async () => {
    const worker = new ScriptedWorker(forge);
    const checker = await attach(worker, init);
    const active = outcome(checker.check({ projectId: "/m/a" }));
    const queued = outcome(checker.check({ projectId: "/m/b" }));
    await flush();
    await flush();
    assert.equal(active.settled, "rejected");
    assert.equal(queued.settled, "rejected");
    assert.match(active.error.message, message);
    assert.equal(queued.error, active.error);
    await assert.rejects(checker.check({ projectId: "/m/c" }), error => error === active.error);
    assert.equal(worker.checks, 1);
    assert.equal(worker.terminated, 1);
  });
}

test("a well-formed refusal resolves and leaves the checker usable", async () => {
  const worker = new ScriptedWorker(({ id }) => ({ type: "result", id, status: 1, output: '{"error":"refused"}', transport: { opened: [] } }));
  const checker = await attach(worker, init);
  const first = await checker.check({ projectId: "/m/a" });
  const second = await checker.check({ projectId: "/m/b" });
  assert.deepEqual([first.ok, first.error, second.ok], [false, "refused", false]);
  assert.equal(worker.terminated, 0);
  await checker.close();
  assert.equal(worker.terminated, 1);
});
