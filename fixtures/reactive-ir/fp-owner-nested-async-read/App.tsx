import { createMemo, createSignal, createTrackedEffect } from "solid-js";

const [id] = createSignal(1);

// Negative: the async IIFE is code the tracked callback runs, not the
// tracked callback. The callback itself is synchronous and finishes before
// the await.
export function AsyncIife() {
  createTrackedEffect(() => {
    void (async () => {
      await fetch("/api/item");
      void id();
    })();
  });
  return <div />;
}

// Negative: the memo's value is an async closure; its body runs when a caller
// invokes it, outside the memo's tracking pass.
export function ReturnedAsyncClosure() {
  const run = createMemo(() => async () => {
    await fetch("/api/item");
    return id();
  });
  return <div>{typeof run()}</div>;
}

// Positive control: the async function IS the computation, so the read after
// the await no longer subscribes.
export function AwaitingComputation() {
  const value = createMemo(async () => {
    await fetch("/api/item");
    return id();
  });
  return <div>{String(value())}</div>;
}
