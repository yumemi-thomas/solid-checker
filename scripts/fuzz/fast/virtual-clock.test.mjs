import { afterEach, describe, expect, it } from "vitest";
import { installVirtualClock } from "./virtual-clock.mjs";

// The clock is installed on a stand-in for the global object, so the test's own timers stay real.
const harness = (onError) => {
  const target = { setTimeout, clearTimeout, setInterval, clearInterval, setImmediate };
  const errors = [];
  const clock = installVirtualClock({ target, onError: onError ?? ((error) => errors.push(error)) });
  return { target, clock, errors };
};
let clock;
afterEach(() => clock?.restore());

describe("the virtual clock", () => {
  it("runs a timer when the clock reaches it, not before, in the order of expiry and then of creation", async () => {
    const h = harness(); clock = h.clock;
    const seen = [];
    h.target.setTimeout(() => seen.push("late"), 50);
    h.target.setTimeout(() => seen.push("a"), 10);
    h.target.setTimeout(() => seen.push("b"), 10);
    h.target.setTimeout(() => seen.push("zero"), 0);
    await h.clock.advance(9);
    expect(seen).toEqual(["zero"]);
    await h.clock.advance(1);
    expect(seen).toEqual(["zero", "a", "b"]);
    await h.clock.advance(40);
    expect(seen).toEqual(["zero", "a", "b", "late"]);
    expect(h.clock.now()).toBe(50);
    expect(h.clock.pending()).toBe(0);
  });

  it("takes Node's delay rules: below 1 ms, NaN, undefined are 1 ms, so setTimeout(f, 0) runs before setTimeout(g, 2) and after nothing earlier", async () => {
    const h = harness(); clock = h.clock;
    const seen = [];
    h.target.setTimeout(() => seen.push("two"), 2);
    h.target.setTimeout(() => seen.push("nan"), Number.NaN);
    h.target.setTimeout(() => seen.push("undefined"));
    h.target.setTimeout(() => seen.push("negative"), -5);
    await h.clock.advance(1);
    expect(seen).toEqual(["nan", "undefined", "negative"]);
    await h.clock.advance(1);
    expect(seen).toEqual(["nan", "undefined", "negative", "two"]);
  });

  it("runs the timers a callback sets within the same advance, a 0 ms chain a millisecond a link, and stops at the limit", async () => {
    const h = harness(); clock = h.clock;
    let links = 0;
    const link = () => { links += 1; h.target.setTimeout(link, 0); };
    h.target.setTimeout(link, 0);
    await h.clock.advance(60);
    expect(links).toBe(60);
    expect(h.clock.pending()).toBe(1);
  });

  it("drains the microtasks (promise chains, queueMicrotask, await) a callback queues before the next timer", async () => {
    const h = harness(); clock = h.clock;
    const seen = [];
    h.target.setTimeout(() => {
      seen.push("timer1");
      Promise.resolve().then(() => seen.push("then1")).then(() => seen.push("then2")).then(() => seen.push("then3"));
      queueMicrotask(() => seen.push("micro"));
      (async () => { await null; await null; seen.push("awaited"); })();
    }, 0);
    h.target.setTimeout(() => seen.push("timer2"), 0);
    await h.clock.advance(5);
    expect(seen.indexOf("timer2")).toBe(seen.length - 1);
    expect(seen).toEqual(expect.arrayContaining(["then3", "micro", "awaited"]));
  });

  it("lets the microtasks of the code that called advance run before the clock moves (a promise settled by the caller)", async () => {
    const h = harness(); clock = h.clock;
    const seen = [];
    h.target.setTimeout(() => seen.push("timer"), 0);
    Promise.resolve().then(() => h.target.setTimeout(() => seen.push("from-microtask"), 0));
    await h.clock.advance(3);
    expect(seen).toEqual(["timer", "from-microtask"]);
  });

  it("clears a timer by handle or by number, and an interval repeats until cleared", async () => {
    const h = harness(); clock = h.clock;
    const seen = [];
    const gone = h.target.setTimeout(() => seen.push("gone"), 5);
    const byNumber = h.target.setTimeout(() => seen.push("gone-too"), 5);
    h.target.clearTimeout(gone);
    h.target.clearTimeout(Number(byNumber));
    let ticks = 0;
    const interval = h.target.setInterval(() => { ticks += 1; if (ticks === 3) h.target.clearInterval(interval); }, 10);
    await h.clock.advance(100);
    expect(seen).toEqual([]);
    expect(ticks).toBe(3);
    expect(h.clock.pending()).toBe(0);
  });

  it("reports a callback that throws and goes on with the rest", async () => {
    const h = harness(); clock = h.clock;
    const seen = [];
    h.target.setTimeout(() => { throw new Error("boom"); }, 0);
    h.target.setTimeout(() => seen.push("after"), 0);
    await h.clock.advance(2);
    expect(h.errors.map((e) => e.message)).toEqual(["boom"]);
    expect(seen).toEqual(["after"]);
  });

  it("puts the real timers back", () => {
    const h = harness();
    expect(h.target.setTimeout).not.toBe(setTimeout);
    h.clock.restore();
    expect(h.target.setTimeout).toBe(setTimeout);
    expect(h.target.clearInterval).toBe(clearInterval);
  });

  it("costs no wall time for a long wait", async () => {
    const h = harness(); clock = h.clock;
    const started = performance.now();
    await h.clock.advance(60_000);
    expect(performance.now() - started).toBeLessThan(500);
    expect(h.clock.now()).toBe(60_000);
  });
});
