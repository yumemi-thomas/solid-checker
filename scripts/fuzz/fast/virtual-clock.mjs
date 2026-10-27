// A virtual clock for the fast runner (`--virtual-clock`): the timers a program sets (setTimeout, setInterval and their clears) run on
// a clock the runner advances, instead of the wall clock, so a "wait 60 ms" costs no time. Nothing else is replaced: microtasks,
// promises, queueMicrotask, setImmediate and process.nextTick are the real ones, and so is Date.
//
// What it keeps of the real thing (the test file is the contract):
//   - Node's delay rules (a delay below 1 ms, NaN or above 2^31-1 is 1 ms), the order of timers (expiry time, then creation order),
//     a timer set while the clock advances expires at the clock's time plus its delay (a chain of 0 ms timers advances by 1 ms a link);
//   - microtasks run between two timer callbacks, as Node runs them: the clock yields a macrotask (the real setImmediate) after
//     every callback and before it returns, and a microtask chain of any length drains before a macrotask runs;
//   - a callback that throws is not the clock's to catch: it is reported as an uncaught exception, as Node reports one from a timer
//     (`onError`), and the remaining timers still run.
// `advance(ms)` is the virtual `await sleep(ms)`: it runs every timer due within `ms` of virtual time, in order, then lets the
// microtasks they queued run.

const MAX_DELAY = 2 ** 31 - 1;
const delayOf = (value) => {
  const delay = Number(value);
  return delay >= 1 && delay <= MAX_DELAY ? Math.trunc(delay) : 1;
};

/**
 * Installs the clock on `target` (the global object). `restore()` puts the real timers back.
 * @param {{ target?: object, onError?: (error: unknown) => void }} [options]
 */
export function installVirtualClock({ target = globalThis, onError = (error) => { throw error; } } = {}) {
  const real = { setTimeout: target.setTimeout, clearTimeout: target.clearTimeout, setInterval: target.setInterval, clearInterval: target.clearInterval };
  const yieldMacrotask = target.setImmediate ? () => new Promise((done) => target.setImmediate(done)) : () => new Promise((done) => real.setTimeout.call(target, done, 0));
  const timers = new Map();
  let now = 0;
  let created = 0;
  let nextId = 1;

  const handleOf = (timer) => ({
    ref() { return this; }, unref() { return this; }, hasRef() { return true; },
    refresh() { timer.at = now + timer.delay; timer.order = created++; return this; },
    close() { timers.delete(timer.id); return this; },
    [Symbol.toPrimitive]() { return timer.id; },
  });
  const schedule = (callback, delay, args, repeat) => {
    if (typeof callback !== "function") throw new TypeError('The "callback" argument must be of type function.');
    const timer = { id: nextId++, delay: delayOf(delay), callback, args, repeat, at: 0, order: created++ };
    timer.at = now + timer.delay;
    timers.set(timer.id, timer);
    return handleOf(timer);
  };
  const clear = (handle) => {
    if (handle === undefined || handle === null) return;
    const id = typeof handle === "object" ? Number(handle[Symbol.toPrimitive]?.()) : Number(handle);
    timers.delete(id);
  };
  const due = (limit) => {
    let first = null;
    for (const timer of timers.values()) if (timer.at <= limit && (first === null || timer.at < first.at || (timer.at === first.at && timer.order < first.order))) first = timer;
    return first;
  };

  target.setTimeout = (callback, delay, ...args) => schedule(callback, delay, args, false);
  target.setInterval = (callback, delay, ...args) => schedule(callback, delay, args, true);
  target.clearTimeout = clear;
  target.clearInterval = clear;

  return {
    /** Virtual milliseconds elapsed. */
    now: () => now,
    /** Timers waiting (a repeating one counts once). */
    pending: () => timers.size,
    /** The virtual `sleep(ms)`: runs the timers due within `ms`, in order, with the microtasks after each; the clock ends `ms` later. */
    async advance(ms) {
      const limit = now + Math.max(0, ms);
      await yieldMacrotask();
      for (let timer = due(limit); timer; timer = due(limit)) {
        now = Math.max(now, timer.at);
        if (timer.repeat) { timer.at = now + timer.delay; timer.order = created++; } else timers.delete(timer.id);
        try { timer.callback(...timer.args); } catch (error) { onError(error); }
        await yieldMacrotask();
      }
      now = limit;
    },
    restore() {
      Object.assign(target, real);
      timers.clear();
    },
  };
}
