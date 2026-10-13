import { createEffect, onCleanup, onSettled } from "solid-js";

function register() {
  onCleanup(() => {});
}

// The `@solid-primitives/timer` shape: which registration runs depends on the
// argument.
function createTimer(fn: () => void, delay: number | (() => number)) {
  if (typeof delay === "number") {
    onCleanup(() => fn());
    return;
  }
  createEffect(() => delay(), () => fn());
}

function guarded(flag: boolean) {
  if (flag) onCleanup(() => {});
}

function afterGuardThrow(ready: boolean) {
  if (!ready) throw new Error("not ready");
  onCleanup(() => {});
}

function looped(count: number) {
  for (let index = 0; index < count; index++) register();
}

function forwardsWhen(flag: boolean) {
  if (flag) register();
}

function forwards() {
  register();
}

function clean(value: number) {
  if (value > 0) console.log(value);
}

// Violation: the helper registers on every completion.
export function Unconditional() {
  onSettled(() => {
    register();
  });
  return <div />;
}

// Uncertifiable: a thrown guard skips the registration, and a caller may
// catch it and carry on.
export function AfterGuardThrow() {
  onSettled(() => {
    afterGuardThrow(true);
  });
  return <div />;
}

// Violation: two helpers down, every hop unconditional.
export function NestedUnconditional() {
  onSettled(() => {
    forwards();
  });
  return <div />;
}

// Uncertifiable, twice: a numeric delay takes the cleanup branch, any other
// the effect; neither runs on every completion of the helper.
export function ArgumentBranch() {
  onSettled(() => {
    createTimer(() => {}, 30_000);
  });
  return <div />;
}

// Uncertifiable: the registration is behind the helper's own branch.
export function Guarded() {
  onSettled(() => {
    guarded(false);
  });
  return <div />;
}

// Uncertifiable: a loop body may run zero times.
export function Looped() {
  onSettled(() => {
    looped(0);
  });
  return <div />;
}

// Uncertifiable: the second hop is conditional in the first.
export function NestedConditional() {
  onSettled(() => {
    forwardsWhen(false);
  });
  return <div />;
}

// Violation, unchanged: a branch written in the leaf callback itself is under
// the statically-present-code premise.
export function LeafBodyBranch(props: { flag: boolean }) {
  onSettled(() => {
    if (props.flag) register();
  });
  return <div />;
}

// Clean: the helper performs no forbidden operation on any path.
export function Clean() {
  onSettled(() => {
    clean(1);
  });
  return <div />;
}

async function maybeWaits(flag: boolean) {
  if (flag) await Promise.resolve();
  onCleanup(() => {});
}

// Uncertifiable: past a conditional `await` the registration runs in the leaf
// scope only when the helper did not suspend first.
export function ConditionalAwait() {
  onSettled(() => {
    void maybeWaits(false);
  });
  return <div />;
}

function registerWhenUnset(target: { cleanup?: () => void }) {
  target.cleanup ??= register;
  target.cleanup ||= (register(), register);
}

// Uncertifiable: `||=` evaluates its right side only when the target is falsy.
export function LogicalAssignment() {
  onSettled(() => {
    registerWhenUnset({});
  });
  return <div />;
}

function fail(message: string): never {
  throw new Error(message);
}

function registerAfterFail(ready: boolean) {
  if (!ready) fail("not ready");
  onCleanup(() => {});
}

// Uncertifiable: a `never`-returning guard may throw into a caller that
// catches it and carries on.
export function AfterFail() {
  onSettled(() => {
    try {
      registerAfterFail(false);
    } catch {
      // carry on
    }
  });
  return <div />;
}

function assertReady(ready: boolean): asserts ready {
  if (!ready) throw new Error("not ready");
}

function registerAfterAssert(ready: boolean) {
  assertReady(ready);
  onCleanup(() => {});
}

// Uncertifiable: an `asserts` guard may throw into a caller that catches it
// (ADR 0279).
export function AfterAssert() {
  onSettled(() => {
    try {
      registerAfterAssert(false);
    } catch {
      // carry on
    }
  });
  return <div />;
}
