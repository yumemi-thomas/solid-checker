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

// ADR 0950: a report from the what-if build (`solid-checker-what-if --assume`: a top-level `assumed` key, or a finding premise of kind `assumed`) is
// a prediction, never a fact about the checker. The owned checker never writes one; a worker that answered one is not one to stand behind, so
// the answer poisons the checker the way a malformed one does, under a message of its own.
const hazard = "OpenMemberWrite(*)";
const finding = extra => ({ id: "SC1001", rule: "strict-read-untracked", kind: "violation", primaryLocation: { path: "/m/a/App.tsx", startByte: 10, endByte: 20, line: 4, column: 3, endLine: 4 }, ...extra });
const withheld = { kind: "assumed", hazards: [hazard] };

for (const [name, snapshot] of [
  ["a top-level assumed key", { status: "violation", findings: [finding()], assumed: { hazards: [hazard], flipped: [] } }],
  ["a finding premise of kind assumed", { status: "violation", findings: [finding({ premises: [withheld] })] }],
  ["both, as the what-if build writes them", { status: "violation", findings: [finding({ premises: [withheld] })], assumed: { hazards: [hazard], flipped: [hazard] } }],
]) {
  test(`${name} (a what-if report) rejects the active, queued and later checks and stops the worker`, async () => {
    const worker = new ScriptedWorker(({ id }) => ({ type: "result", id, status: 0, output: JSON.stringify({ snapshot, notes: [] }), transport: {} }));
    const checker = await attach(worker, init);
    const active = outcome(checker.check({ projectId: "/m/a" }));
    const queued = outcome(checker.check({ projectId: "/m/b" }));
    await flush();
    await flush();
    assert.equal(active.settled, "rejected");
    assert.equal(queued.settled, "rejected");
    assert.equal(active.error.name, "OwnedCheckerError");
    assert.match(active.error.message, /^the owned checker answered a report tainted by a what-if assumption, .* Only scripts\/what-if\.mjs reads one\.$/);
    assert.equal(queued.error, active.error);
    await assert.rejects(checker.check({ projectId: "/m/c" }), error => error === active.error);
    assert.equal(worker.checks, 1);
    assert.equal(worker.terminated, 1);
  });
}

test("a report as a default build writes it, premises of other kinds included, is the answer unchanged", async () => {
  const snapshot = { status: "violation", findings: [finding({ premises: [{ kind: "SiteReachAbsentProp" }] })] };
  const worker = new ScriptedWorker(({ id }) => ({ type: "result", id, status: 0, output: JSON.stringify({ snapshot, notes: ["a note"] }), transport: { opened: [] } }));
  const checker = await attach(worker, init);
  const answer = await checker.check({ projectId: "/m/a" });
  assert.deepEqual([answer.ok, answer.snapshot, answer.notes], [true, snapshot, ["a note"]]);
  assert.equal(worker.terminated, 0);
  await checker.close();
  assert.equal(worker.terminated, 1);
});
