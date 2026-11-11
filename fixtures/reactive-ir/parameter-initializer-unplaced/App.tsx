import { action, createEffect, createMemo, createSignal, flush } from "solid-js";

const [n] = createSignal(0);

// ADR 0961 (PD): a site written in a parameter initializer (a default value, a destructuring default, an IIFE, a closure or a
// helper call in one) runs when a call of its function omits the argument. No walk here reads which calls omit it, so the
// site is an uncertifiable row of the rule that asks, never a violation and never silence. Chrome (rc.21) fires
// FLUSH_IN_ACTION at each of these flushes when the action runs, and PRIMITIVE_IN_EFFECT_CALLBACK / FLUSH_IN_EFFECT_CALLBACK
// at the ones the apply calls.

// The action's step calls a function whose default flushes: the default itself, an IIFE, a closure, a helper's call,
// a destructuring default, a generator's default.
function value(x: unknown = flush()) {
  return x;
}
export const viaValue = action(function* () {
  value();
  yield Promise.resolve();
});

const arrow = (x: unknown = flush()) => x;
export const viaArrow = action(function* () {
  arrow();
  yield Promise.resolve();
});

function iife(x: unknown = (() => {
  flush();
  return 1;
})()) {
  return x;
}
export const viaIife = action(function* () {
  iife();
  yield Promise.resolve();
});

function closure(f: () => void = () => flush()) {
  f();
}
export const viaClosure = action(function* () {
  closure();
  yield Promise.resolve();
});

function flushes() {
  flush();
}
function helper(x: unknown = flushes()) {
  return x;
}
export const viaHelper = action(function* () {
  helper();
  yield Promise.resolve();
});

function destructured({ v = flush() }: { v?: unknown } = {}) {
  return v;
}
export const viaDestructured = action(function* () {
  destructured();
  yield Promise.resolve();
});

function* generatorDefault(x: unknown = (() => {
  flush();
  return 1;
})()) {
  yield x;
}
export const viaGenerator = action(function* () {
  yield* generatorDefault();
});

// A method's default: the same row.
class Box {
  method(x: unknown = flush()) {
    return x;
  }
}
const box = new Box();
export const viaMethod = action(function* () {
  box.method();
  yield Promise.resolve();
});

// The argument is passed, so the default runs nothing (Chrome: quiet). The row stays: which calls omit the argument is not read.
function passed(x: unknown = flush()) {
  return x;
}
export const viaPassed = action(function* () {
  passed(1);
  yield Promise.resolve();
});

// Silent: the default of the action's own step generator is evaluated when the action creates the generator, before the first
// step (ADR 0454), and a function nothing calls or names runs its default nowhere.
export const ownDefault = action(function* (x: unknown = flush()) {
  yield x;
});
function unused(x: unknown = flush()) {
  return x;
}

// The effect's apply calls a function whose default flushes, and one whose default creates a computation.
function applyFlush(x: unknown = flush()) {
  return x;
}
function applyMemo(x: unknown = createMemo(() => 1)) {
  return x;
}
export function InApply() {
  createEffect(
    () => n(),
    () => {
      applyFlush();
      applyMemo();
    },
  );
  return <div />;
}

export function App() {
  return (
    <button
      onClick={() => {
        void viaValue();
        void viaArrow();
        void viaIife();
        void viaClosure();
        void viaHelper();
        void viaDestructured();
        void viaGenerator();
        void viaMethod();
        void viaPassed();
        void ownDefault();
      }}
    >
      go
    </button>
  );
}
