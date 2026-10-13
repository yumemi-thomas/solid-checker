// SC2006 flush-in-action. From @solidjs/signals@2.0.0-rc.8, `flush` opens with
//
//   if (actionStepDepth > 0) throw new Error("[FLUSH_IN_ACTION] …")
//
// (rc.9 `dist/dev-shared.js:2210-2219`), and `action`'s `step` raises that
// depth only around the generator's own `it.next(v)` (`dist/dev.js:2016-2024`).
// So the call throws in dev exactly while a step of the body is running. The
// production build takes the same branch and skips the drain.
import { action, flush, untrack } from "solid-js";
import * as Solid from "solid-js";

// Positive: the head of a sync generator body.
export const syncHead = action(function* (text: string) {
  flush();
  yield text;
});

// Positive: `flush(fn)` is refused the same way; the guard precedes `fn`.
export const syncHeadWithCallback = action(function* () {
  flush(() => 1);
});

// Positive: a sync generator resumes inside the next step after a yielded
// promise, so the rest of its body is still inside a step.
export const syncAfterYieldedPromise = action(function* () {
  yield Promise.resolve(1);
  flush();
});

// Positive: an async generator's first step runs to its first suspension.
export const asyncHead = action(async function* (id: number) {
  flush();
  const saved = await Promise.resolve(id);
  yield saved;
});

// Positive: the awaited operand is evaluated before the await suspends.
export const asyncAwaitedOperand = action(async function* () {
  await flush(() => Promise.resolve(1));
});

// Positive: a namespace import resolves to the same export.
export const namespaced = action(function* () {
  Solid.flush();
});

// Negative: after an await, the body continues from a microtask, outside any
// step.
export const asyncAfterAwait = action(async function* () {
  await Promise.resolve();
  flush();
});

// Negative: the await in the argument runs before the call.
export const asyncAwaitInArgument = action(async function* () {
  flush(await Promise.resolve(() => 1));
});

// Negative: a `for await` awaits before its body runs.
export const asyncForAwait = action(async function* (source: AsyncIterable<number>) {
  for await (const value of source) {
    flush();
    yield value;
  }
});

// Negative: after an async `yield*`, the body resumes from a microtask.
export const asyncAfterDelegation = action(async function* (inner: () => AsyncGenerator<number>) {
  yield* inner();
  flush();
});

// Negative (not claimed): the first iteration throws, a later one runs after
// the await, and the proof does not separate them.
export const asyncLoop = action(async function* () {
  for (let i = 0; i < 2; i++) {
    flush();
    await Promise.resolve();
  }
});

// Negative: a parameter initializer runs when the generator is created, before
// the first step.
export const parameterInitializer = action(function* (settled = flush(() => true)) {
  yield settled;
});

// Negative: callbacks the body schedules run after the step returns.
export const scheduled = action(function* () {
  setTimeout(() => flush(), 0);
  void Promise.resolve().then(() => flush());
});

// Negative (not claimed): the probes show this throws -- `untrack` runs its
// callback inline -- but the proof is lexical and stops at the nested function.
export const inUntrack = action(function* () {
  untrack(() => flush());
});

// Negative: the argument is not the generator, so what is stepped is whatever
// `wrap` returns.
function wrap<Args extends any[], Y, R>(genFn: (...args: Args) => Generator<Y, R, any>) {
  return genFn;
}
export const wrapped = action(
  wrap(function* () {
    flush();
  })
);

// Negative: a generator nobody hands to `action` is not an action body.
export function* plainGenerator() {
  flush();
  yield 1;
}

// Negative: a local `flush` shadows the import.
export function shadowed(flush: () => void) {
  return action(function* () {
    flush();
  });
}

// Negative: outside an action, flush() drains as usual.
export function reveal() {
  flush();
}

// Negative: an event handler that runs an action and then drains.
export function SaveButton() {
  return (
    <button
      onClick={() => {
        void syncHead("draft");
        flush();
      }}
    >
      save
    </button>
  );
}
