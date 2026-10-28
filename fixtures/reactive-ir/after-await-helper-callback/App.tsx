import { createEffect, createMemo, createSignal } from "solid-js";
import { runNow } from "./helpers";

// ADR 0485: a callback a project helper calls in its own synchronous body runs
// on the continuation's stack, so a read in it after the computation's await
// is untracked (SC1002), as a direct read there is.

function run(cb: () => unknown) {
  cb();
  return 1;
}

const kept: Array<() => unknown> = [];
function keep(cb: () => unknown) {
  kept.push(cb);
  return 1;
}

const pause = () => Promise.resolve();

// Violation: through a same-file helper.
export function ThroughHelper() {
  const [count] = createSignal(0);
  const m = createMemo(async () => {
    await pause();
    run(() => count());
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div></div>;
}

// Violation: through a helper in another module.
export function ThroughModuleHelper() {
  const [count] = createSignal(0);
  const m = createMemo(async () => {
    await pause();
    runNow(() => count());
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div></div>;
}

// Not claimed: a helper that keeps its callback may call it anywhere.
export function KeptCallback() {
  const [count] = createSignal(0);
  const m = createMemo(async () => {
    await pause();
    keep(() => count());
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div></div>;
}

// Silent: before the await the read is tracked.
export function BeforeAwait() {
  const [count] = createSignal(0);
  const m = createMemo(async () => {
    run(() => count());
    await pause();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div></div>;
}
