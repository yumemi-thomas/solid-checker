// SC2006 through function layers that run in place in an action step (ADR 0478):
// the call runs on the stack of the outermost layer's call, which is in the step.
import { action, flush } from "solid-js";
import { runNow } from "./helpers";

function run(cb: () => unknown) {
  cb();
  return 1;
}

const saved: Array<() => unknown> = [];
function keep(cb: () => unknown) {
  saved.push(cb);
  return 1;
}

// Positive: an IIFE in a sync generator body.
export const viaIife = action(function* () {
  (() => flush())();
  yield 1;
});

// Positive: an inline standard-library callback.
export const viaForEach = action(function* () {
  [1].forEach(() => flush());
});

// Positive: a same-file helper that calls its callback in its body.
export const viaHelper = action(function* () {
  run(() => flush());
});

// Positive: the helper in another module.
export const viaModuleHelper = action(function* () {
  runNow(() => flush());
});

// Positive: two layers, in an async generator before its first suspension.
export const asyncNested = action(async function* () {
  (() => run(() => flush()))();
  await Promise.resolve(1);
});

// Negative: the IIFE runs after the await, outside the first step.
export const afterAwait = action(async function* () {
  await Promise.resolve(1);
  (() => flush())();
});

// Negative: the call follows the IIFE's own await, on a later task.
export const asyncIife = action(function* () {
  void (async () => {
    await Promise.resolve(1);
    flush();
  })();
});

// Negative: a helper that keeps its callback does not run it here.
export const kept = action(function* () {
  keep(() => flush());
});

// Negative: a scheduled callback inside the layer runs on a fresh stack.
export const scheduled = action(function* () {
  (() => setTimeout(() => flush(), 0))();
});

// Negative: an IIFE in the generator's parameter list runs before the first step.
export const inParameterList = action(function* (x: number = (() => { flush(); return 1; })()) {
  yield x;
});

// Positive (ADR 0479): a named closure whose only reference is one call runs on that call's stack.
export const namedClosure = action(function* () {
  const f = () => flush();
  f();
});
