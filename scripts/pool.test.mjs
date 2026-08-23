import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  createWorkerPool,
  gateConcurrency,
  mapPool,
  mapPoolSettled,
  recommendedConcurrency,
} from "./lib/pool.mjs";

const settle = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

test("recommended concurrency follows the cores, capped at eight", () => {
  assert.equal(recommendedConcurrency(1), 1);
  assert.equal(recommendedConcurrency(4), 4);
  assert.equal(recommendedConcurrency(14), 8);
  assert.equal(recommendedConcurrency(128), 8);
  // A host that cannot report its parallelism gets a conservative constant
  // rather than an unbounded fan-out. (Omitting the argument entirely asks the
  // OS instead, which is the production path.)
  assert.equal(recommendedConcurrency(null), 4);
  assert.equal(recommendedConcurrency(Number.NaN), 4);
  assert.equal(recommendedConcurrency(0), 4);
  assert.equal(recommendedConcurrency(-1), 4);
});

test("an explicit concurrency override is honoured, a malformed one is refused", () => {
  assert.equal(gateConcurrency({ SOLID_CHECKER_GATE_CONCURRENCY: "3" }), 3);
  assert.equal(gateConcurrency({ SOLID_CHECKER_GATE_CONCURRENCY: "" }), recommendedConcurrency());
  assert.equal(gateConcurrency({}), recommendedConcurrency());
  // Silently serializing on a typo would look exactly like a slow machine.
  for (const bad of ["0", "-2", "eight", "2.5"]) {
    assert.throws(
      () => gateConcurrency({ SOLID_CHECKER_GATE_CONCURRENCY: bad }),
      /must be a positive integer/,
    );
  }
});

test("results come back in item order however the units finish", async () => {
  const items = [0, 1, 2, 3, 4, 5, 6, 7];
  const finished = [];
  const values = await mapPool(
    items,
    async (item) => {
      // Deliberately inverted: the last item finishes first.
      await settle((items.length - item) * 4);
      finished.push(item);
      return item * 10;
    },
    { concurrency: 8 },
  );

  assert.deepEqual(values, [0, 10, 20, 30, 40, 50, 60, 70]);
  assert.notDeepEqual(finished, items, "the test would prove nothing if completion order matched");
});

test("no more than `concurrency` units are ever in flight", async () => {
  let inFlight = 0;
  let peak = 0;
  await mapPool(
    Array.from({ length: 40 }, (_, index) => index),
    async () => {
      inFlight += 1;
      peak = Math.max(peak, inFlight);
      await settle(2);
      inFlight -= 1;
    },
    { concurrency: 3 },
  );

  assert.equal(peak, 3);
  assert.equal(inFlight, 0);
});

test("a single unit still runs when concurrency exceeds the work", async () => {
  const values = await mapPool([7], async (item) => item, { concurrency: 8 });
  assert.deepEqual(values, [7]);
  assert.deepEqual(await mapPool([], async () => 1, { concurrency: 8 }), []);
});

test("the lowest-indexed failure is the one thrown, whichever failed first", async () => {
  const started = [];
  await assert.rejects(
    mapPool(
      [0, 1, 2, 3, 4, 5],
      async (item) => {
        started.push(item);
        // Item 3 fails immediately, item 1 fails later: a sequential loop
        // would have died on 1, so that is the error a run must report.
        if (item === 3) throw new Error("failure at 3");
        if (item === 1) {
          await settle(20);
          throw new Error("failure at 1");
        }
        await settle(5);
        return item;
      },
      { concurrency: 6 },
    ),
    /failure at 1/,
  );
  assert.deepEqual(started.sort((a, b) => a - b), [0, 1, 2, 3, 4, 5]);
});

test("a failure stops handing out further units", async () => {
  const started = [];
  await assert.rejects(
    mapPool(
      Array.from({ length: 30 }, (_, index) => index),
      async (item) => {
        started.push(item);
        await settle(2);
        if (item === 0) throw new Error("first unit fails");
        return item;
      },
      { concurrency: 2 },
    ),
    /first unit fails/,
  );
  assert.ok(started.length < 10, `expected an early stop, started ${started.length} units`);
});

test("settling every unit reports one verdict per item, in item order", async () => {
  const settled = await mapPoolSettled(
    [0, 1, 2, 3],
    async (item) => {
      if (item % 2 === 1) throw new Error(`odd ${item}`);
      return item;
    },
    { concurrency: 4 },
  );

  assert.deepEqual(
    settled.map(({ index, value, error }) => [index, value, error?.message]),
    [
      [0, 0, undefined],
      [1, undefined, "odd 1"],
      [2, 2, undefined],
      [3, undefined, "odd 3"],
    ],
  );
});

test("a worker-thread pool keeps order, bounds its threads, and reports a thrown task", async () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-pool-test-"));
  try {
    const workerPath = join(directory, "worker.mjs");
    writeFileSync(
      workerPath,
      [
        'import { parentPort, threadId } from "node:worker_threads";',
        'parentPort.on("message", ({ id, payload }) => {',
        "  if (payload.explode) {",
        '    parentPort.postMessage({ id, ok: false, error: { message: `boom ${payload.value}` } });',
        "    return;",
        "  }",
        "  parentPort.postMessage({ id, ok: true, value: { doubled: payload.value * 2, threadId } });",
        "});",
        "",
      ].join("\n"),
    );

    const pool = createWorkerPool({ workerPath, size: 3 });
    try {
      const observed = await mapPool(
        Array.from({ length: 24 }, (_, index) => index),
        (value) => pool.run({ value }),
        { concurrency: 3 },
      );
      assert.deepEqual(
        observed.map((entry) => entry.doubled),
        Array.from({ length: 24 }, (_, index) => index * 2),
      );
      assert.ok(
        new Set(observed.map((entry) => entry.threadId)).size <= 3,
        "the pool must never exceed its declared size",
      );

      await assert.rejects(pool.run({ value: 9, explode: true }), /boom 9/);
      // A rejected task must not poison the worker that served it: the next
      // task gets a real answer, computed on a thread the pool already owns.
      const after = await pool.run({ value: 21 });
      assert.equal(after.doubled, 42);
      assert.ok(Number.isInteger(after.threadId) && after.threadId > 0);
    } finally {
      await pool.close();
    }
    await assert.rejects(pool.run({ value: 1 }), /closed/);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

/** A pool test must never be able to hang the suite: every wait is bounded. */
const within = async (promise, ms, what) => {
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error(`${what} did not settle within ${ms}ms`)), ms);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
};

/**
 * Attach the handlers at creation, so a task that rejects while the test is
 * still setting up the next one is never an unhandled rejection.
 */
const outcome = (promise) =>
  promise.then(
    (value) => ({ value }),
    (error) => ({ error: error.message }),
  );

const withWorker = async (source, body) => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-pool-test-"));
  try {
    const workerPath = join(directory, "worker.mjs");
    writeFileSync(workerPath, `${source}\n`);
    return await body(workerPath);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
};

test("an unattributable message never answers another case, and retires the worker", async () => {
  // The shape scripts/lib/tsc-oracle-gate-worker.mjs's uncaughtException
  // handler used to post: `{id: null}`, while a *different* case is in flight.
  // Ignoring the id applies it to that case -- and then hands the still-busy
  // worker back out as idle, so the case after it receives the previous case's
  // value. Both halves of that are wrong verdicts, in either direction.
  await withWorker(
    [
      'import { parentPort } from "node:worker_threads";',
      'parentPort.on("message", ({ id, payload }) => {',
      '  if (payload.label === "CASE-A") {',
      "    parentPort.postMessage({ id, ok: true, value: 'CASE-A-result' });",
      "    setTimeout(",
      '      () => parentPort.postMessage({ id: null, ok: false, error: { message: "stray fault from CASE-A" } }),',
      "      40,",
      "    );",
      "    return;",
      "  }",
      "  setTimeout(() => parentPort.postMessage({ id, ok: true, value: `${payload.label}-result` }), 300);",
      "});",
    ].join("\n"),
    async (workerPath) => {
      const pool = createWorkerPool({ workerPath, size: 1 });
      try {
        assert.equal(await within(pool.run({ label: "CASE-A" }), 5000, "CASE-A"), "CASE-A-result");

        const b = outcome(pool.run({ label: "CASE-B" }));
        await settle(120); // the stray has landed by now
        const c = outcome(pool.run({ label: "CASE-C" }));

        // CASE-B may not be answered with the stray's content, and may not be
        // left unsettled either: it fails, naming what happened.
        const bSettled = await within(b, 5000, "CASE-B");
        assert.match(
          bSettled.error ?? "",
          /no longer attributable/,
          `a stray message must not become CASE-B's verdict (got ${JSON.stringify(bSettled)})`,
        );
        // ...and CASE-C gets its own value, from a replacement thread -- never
        // CASE-B's, which is what a double-released worker delivers.
        assert.deepEqual(await within(c, 5000, "CASE-C"), { value: "CASE-C-result" });
      } finally {
        await pool.close();
      }
    },
  );
});

test("a worker that dies while idle is not handed to the next task", async () => {
  // `live.delete(state)` without splicing `idle` leaves a dead state in the
  // idle list. `idle.pop()` hands it out, `postMessage` on a dead thread is
  // silently dropped, and the task's promise never settles -- `make verify`
  // stops at `step tsc-oracle-gate` with no output and no timeout. The code's
  // own comment says "a gate must fail loudly, never quietly stall".
  await withWorker(
    [
      'import { parentPort } from "node:worker_threads";',
      'parentPort.on("message", ({ id, payload }) => {',
      "  parentPort.postMessage({ id, ok: true, value: payload.value * 2 });",
      "  // Stands in for any async fault: OOM, native crash, uncaught async throw.",
      '  if (payload.value === 1) setTimeout(() => { throw new Error("late worker fault"); }, 10);',
      "});",
    ].join("\n"),
    async (workerPath) => {
      const pool = createWorkerPool({ workerPath, size: 1 });
      try {
        assert.equal(await within(pool.run({ value: 1 }), 5000, "task 1"), 2);
        await settle(150); // the worker dies while sitting in `idle`
        // Either a replacement runs it or it fails loudly. Never a stall.
        assert.equal(await within(pool.run({ value: 2 }), 5000, "task 2"), 4);
      } finally {
        await pool.close();
      }
    },
  );
});

test("closing the pool settles every queued task instead of abandoning it", async () => {
  // The internal queue exists "so an over-subscribed caller waits instead of
  // losing a task". A close that terminates the threads and leaves the queue
  // untouched loses them exactly as silently.
  await withWorker(
    [
      'import { parentPort } from "node:worker_threads";',
      'parentPort.on("message", () => {',
      "  // Never answers: the point is what happens to the tasks behind it.",
      "});",
    ].join("\n"),
    async (workerPath) => {
      const pool = createWorkerPool({ workerPath, size: 1 });
      const tasks = [
        outcome(pool.run({ n: 1 })), // in flight
        outcome(pool.run({ n: 2 })), // queued
        outcome(pool.run({ n: 3 })), // queued
      ];
      await settle(50);
      await pool.close();
      for (const [index, task] of tasks.entries()) {
        const settled = await within(task, 5000, `task ${index + 1}`);
        assert.match(settled.error ?? "", /closed/, `task ${index + 1}: ${JSON.stringify(settled)}`);
      }
    },
  );
});

test("a worker that declares its answer fatal serves no further case", async () => {
  // The parent believing `fatal` is what keeps a doomed thread -- one whose
  // `uncaughtException` handler has already fired -- from being handed the next
  // case only to die under it.
  await withWorker(
    [
      'import { parentPort, threadId } from "node:worker_threads";',
      'parentPort.on("message", ({ id, payload }) => {',
      "  if (payload.fault) {",
      '    parentPort.postMessage({ id, ok: false, fatal: true, error: { message: "worker fault" } });',
      "    return;",
      "  }",
      "  parentPort.postMessage({ id, ok: true, value: threadId });",
      "});",
    ].join("\n"),
    async (workerPath) => {
      const pool = createWorkerPool({ workerPath, size: 1 });
      try {
        const first = await within(pool.run({}), 5000, "first");
        await assert.rejects(() => within(pool.run({ fault: true }), 5000, "fault"), /worker fault/);
        const second = await within(pool.run({}), 5000, "second");
        assert.notEqual(second, first, "a fatal fault must not leave the same thread in service");
      } finally {
        await pool.close();
      }
    },
  );
});
