import { createTrackedEffect, flush, isPending, onSettled } from "solid-js";

// Slice 5's lead (batch 11 research; Chrome rc.21 confirmed it a false clean on main 3b22442d4). `isPending(fn)` takes what `fn` throws,
// so a `flush()` in a synchronous callback of a leaf owner (`createTrackedEffect`, `onSettled`) throws nothing the program sees. An async
// callback's synchronous throw is the rejection of the promise it returns, which `isPending` does not catch (ADR 0960 measured the literal
// form): Chrome throws `[FLUSH_IN_TRACKED_QUEUE_CALLBACK]`. The literal form was ADR 0959's obligation; a callback NAMED at module scope went
// through another reader (`slot_callback_operations`), which took the slot's swallowing for every callback, and the program was certified
// clean. An async callback, named or reached through a function the callback calls, is an uncertifiable row of SC3001 at the `isPending`
// call (`leaf-owner-unproven`), never a violation (ADR 0959's coarse rule) and never silence.

// The named async callback, an arrow and a function declaration.
const named = async () => {
  flush();
};
export function NamedAsync() {
  createTrackedEffect(() => {
    isPending(named);
  });
  return <div />;
}

async function declared() {
  flush();
}
export function DeclaredAsync() {
  createTrackedEffect(() => {
    isPending(declared);
  });
  return <div />;
}

// A synchronous arrow calls an async function: it returns the rejected promise.
async function rejects() {
  flush();
}
export function WrappedAsync() {
  createTrackedEffect(() => {
    isPending(() => rejects());
  });
  return <div />;
}

// The other leaf owner.
async function settled() {
  flush();
}
export function SettledNamedAsync() {
  onSettled(() => {
    isPending(settled);
  });
  return <div />;
}

// Silent. A synchronous callback: the throw is its own, which `isPending` takes (named, and one call down).
function plain() {
  flush();
}
export function NamedSync() {
  createTrackedEffect(() => {
    isPending(plain);
  });
  return <div />;
}
export function WrappedSync() {
  createTrackedEffect(() => {
    isPending(() => plain());
  });
  return <div />;
}

// Silent. The flush follows an `await`: it runs after the callback returned its promise, off the leaf owner's stack.
async function later() {
  await Promise.resolve();
  flush();
}
export function NamedAfterAwait() {
  createTrackedEffect(() => {
    isPending(later);
  });
  return <div />;
}
