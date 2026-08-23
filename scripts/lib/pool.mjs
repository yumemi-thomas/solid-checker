// Bounded concurrency for the verification gates.
//
// The gates fan out over independent units -- 83 fixture projects, 161 oracle
// cases, 27 contract fixtures -- and ran them one at a time. Nothing about a
// unit depends on another, so the cost was pure serialization. What a gate
// cannot afford to lose in exchange is determinism: a failure list whose order
// depends on which unit finished first is a failure list nobody can diff.
//
// So every helper here separates *completion order* from *report order*.
// Results are written into a pre-sized array by index, and errors are reported
// lowest-index-first, exactly as a sequential loop would have hit them.
//
// `scripts/ecosystem-benchmark/run.mjs` had the same need first and its
// `mapConcurrent`/`recommendedConcurrency` are the shape reused here; the
// benchmark keeps its own copy because it is a separately-versioned corpus
// runner, not a verification gate.
import { availableParallelism } from "node:os";
import process from "node:process";
import { Worker } from "node:worker_threads";

/**
 * Default fan-out: one unit per core, capped at 8.
 *
 * Every unit launches at least one checker process, which launches a TypeFacts
 * producer, so unconstrained fan-out multiplies a process tree rather than
 * saturating cores. The cap is the same one the ecosystem benchmark settled on
 * for the same reason.
 */
export function recommendedConcurrency(parallelism = availableParallelism()) {
  return Number.isInteger(parallelism) && parallelism > 0 ? Math.min(8, parallelism) : 4;
}

/**
 * Concurrency for a gate: an explicit environment override, else the default.
 *
 * A non-numeric or non-positive override is a hard error rather than a silent
 * fallback -- a typo that quietly serialized the gate would look exactly like
 * the machine being slow.
 */
export function gateConcurrency(env = process.env, variable = "SOLID_CHECKER_GATE_CONCURRENCY") {
  const raw = env[variable];
  if (raw === undefined || raw === "") return recommendedConcurrency();
  const value = Number(raw);
  if (!Number.isInteger(value) || value < 1) {
    throw new Error(`${variable} must be a positive integer, got ${JSON.stringify(raw)}`);
  }
  return value;
}

/**
 * Run `worker` over `items` with at most `concurrency` in flight, settling
 * every unit.
 *
 * Returns one entry per item, in `items` order, shaped
 * `{index, value}` or `{index, error}`. Nothing is thrown: a gate that
 * aggregates per-unit failures into a report needs every unit's verdict, not
 * the first one.
 */
export async function mapPoolSettled(items, worker, { concurrency } = {}) {
  const list = [...items];
  const results = new Array(list.length);
  const size = Math.max(1, Math.min(concurrency ?? recommendedConcurrency(), list.length || 1));
  let cursor = 0;
  const runners = Array.from({ length: size }, async () => {
    while (cursor < list.length) {
      const index = cursor;
      cursor += 1;
      try {
        results[index] = { index, value: await worker(list[index], index) };
      } catch (error) {
        results[index] = { index, error };
      }
    }
  });
  await Promise.all(runners);
  return results;
}

/**
 * Run `worker` over `items` with at most `concurrency` in flight, returning
 * values in `items` order.
 *
 * On failure this stops handing out new units and rethrows the
 * **lowest-indexed** error once the in-flight ones have settled. That is the
 * error a sequential loop would have died on, so a crash reproduces the same
 * message whatever the scheduling did -- and the units after it are left
 * unrun, as they were before.
 */
export async function mapPool(items, worker, options = {}) {
  const list = [...items];
  const results = new Array(list.length);
  const failures = [];
  const size = Math.max(1, Math.min(options.concurrency ?? recommendedConcurrency(), list.length || 1));
  let cursor = 0;
  const runners = Array.from({ length: size }, async () => {
    while (cursor < list.length && failures.length === 0) {
      const index = cursor;
      cursor += 1;
      try {
        results[index] = await worker(list[index], index);
      } catch (error) {
        failures.push({ index, error });
      }
    }
  });
  await Promise.all(runners);
  if (failures.length > 0) {
    failures.sort((a, b) => a.index - b.index);
    throw failures[0].error;
  }
  return results;
}

/**
 * A fixed pool of worker threads, for units whose cost is *in this process*.
 *
 * `mapPool` alone parallelizes child-process work, because waiting on a child
 * yields the event loop. It does nothing for a unit that builds a TypeScript
 * program in-process: that is one thread's CPU, and 161 of them are 161 turns
 * of the same thread. Each worker thread gets its own module registry, hence
 * its own `typescript` instance, so the compiler is never shared across
 * threads.
 *
 * Tasks are dispatched to an idle worker; with `mapPool` bounded to `size`
 * there is always one, and the internal queue stays empty. It exists anyway so
 * an over-subscribed caller waits instead of losing a task.
 *
 * Two invariants matter more than throughput, because breaking either produces
 * a *wrong verdict* rather than a slow one:
 *
 *   1. **A result belongs to exactly the task whose id it carries.** Every
 *      dispatch stamps `id`; a message whose `id` is not the id of the task
 *      currently in flight is unattributable, and an unattributable message
 *      from a thread means the thread's own bookkeeping is gone. Such a worker
 *      is retired (terminated, removed from both pools) and its task fails
 *      loudly. Answering the in-flight task with a stray message instead --
 *      which is what ignoring `id` amounts to -- delivers one case's verdict to
 *      another case and then hands a still-busy worker back out as idle.
 *   2. **Every task settles.** A worker that dies -- while running a task or
 *      while sitting in `idle` -- leaves both pools, and the queue is either
 *      served by a replacement thread or rejected outright. A dead state left
 *      in `idle` is handed to the next task, `postMessage` on it is silently
 *      dropped, and the gate stalls with no output at all. `close()` rejects
 *      whatever is still queued for the same reason.
 */
export function createWorkerPool({ workerPath, size, workerData }) {
  const count = Math.max(1, size);
  const idle = [];
  const queue = [];
  const live = new Set();
  let nextId = 0;
  let closed = false;

  const removeIdle = (state) => {
    const at = idle.indexOf(state);
    if (at !== -1) idle.splice(at, 1);
  };

  /**
   * Hand queued tasks to whatever can run them, and fail them loudly when
   * nothing can.
   *
   * `live.size === 0` is the only state in which waiting is hopeless: any live
   * worker either releases (which pumps again) or dies (which retires and pumps
   * again), so a non-empty queue with a live worker is always making progress.
   */
  const pump = (cause) => {
    while (queue.length > 0 && !closed) {
      let state = idle.pop();
      if (!state && live.size < count) {
        try {
          state = spawn();
        } catch (error) {
          while (queue.length > 0) queue.shift().reject(error);
          return;
        }
      }
      if (!state) break;
      dispatch(state, queue.shift());
    }
    if (queue.length > 0 && (closed || live.size === 0)) {
      const failure =
        cause ?? new Error("gate worker pool has no live worker left to run its queued tasks");
      while (queue.length > 0) queue.shift().reject(failure);
    }
  };

  /**
   * Take a worker out of service: its in-flight task fails, it leaves `idle`
   * and `live` both, and the queue is re-pumped.
   *
   * Removing it from `idle` is the part that matters. `live.delete` alone leaves
   * a dead state in the idle list, and `idle.pop()` will hand it to the next
   * task -- which then never settles.
   */
  const retire = (state, error) => {
    // `terminate()` below fires `exit`, which retires again; the flag keeps the
    // second pass from re-pumping with a stale cause.
    if (state.retired) return;
    state.retired = true;
    removeIdle(state);
    live.delete(state);
    const task = state.task;
    state.task = null;
    if (task) task.reject(error);
    Promise.resolve(state.worker.terminate()).catch(() => {});
    pump(error);
  };

  const spawn = () => {
    const worker = new Worker(workerPath, { workerData });
    const state = { worker, task: null, retired: false };
    worker.on("message", (message) => {
      const task = state.task;
      // An id that is not the in-flight task's id cannot be answered. It is
      // either an unprompted message or a reply to a task that already
      // settled; either way the thread is no longer accounted for, so it goes
      // rather than being trusted with another case.
      if (!task || message?.id !== task.id) {
        retire(
          state,
          new Error(
            `gate worker returned a result for ${JSON.stringify(message?.id ?? null)} while ` +
              `${task ? JSON.stringify(task.id) : "no task"} was in flight; the worker is ` +
              `no longer attributable and has been terminated`,
          ),
        );
        return;
      }
      state.task = null;
      if (message.ok) task.resolve(message.value);
      else {
        const error = new Error(message?.error?.message ?? "worker task failed");
        if (message?.error?.stack) error.stack = message.error.stack;
        task.reject(error);
      }
      // A worker that declares its own answer fatal has told us it cannot serve
      // another case; believing it is cheaper than discovering it on the next.
      if (message.fatal) retire(state, new Error("gate worker reported a fatal fault"));
      else release(state);
    });
    // A thread that dies takes its task with it. Rejecting rather than hanging
    // is the whole point: a gate must fail loudly, never quietly stall.
    worker.on("error", (error) => retire(state, error));
    worker.on("exit", (code) => {
      if (closed) {
        removeIdle(state);
        live.delete(state);
        return;
      }
      // Even a clean exit is a death here: the pool owns the thread's lifetime,
      // so a worker that leaves on its own is one the pool must stop offering.
      retire(state, new Error(`gate worker exited with code ${code}`));
    });
    live.add(state);
    return state;
  };

  const release = (state) => {
    idle.push(state);
    pump();
  };

  const dispatch = (state, task) => {
    state.task = task;
    state.worker.postMessage({ id: task.id, payload: task.payload });
  };

  return {
    size: count,
    run(payload) {
      if (closed) return Promise.reject(new Error("gate worker pool is closed"));
      return new Promise((resolve, reject) => {
        const task = { id: (nextId += 1), payload, resolve, reject };
        queue.push(task);
        pump();
      });
    },
    async close() {
      closed = true;
      // Queued tasks settle before the threads go: a caller awaiting one must
      // get an error, not a promise nobody will ever resolve.
      const stopped = new Error("gate worker pool is closed");
      while (queue.length > 0) queue.shift().reject(stopped);
      const states = [...live];
      for (const state of states) {
        const task = state.task;
        state.task = null;
        if (task) task.reject(stopped);
      }
      await Promise.all(states.map((state) => state.worker.terminate()));
      idle.length = 0;
      live.clear();
    },
  };
}
