import { createEffect, createMemo, createSignal } from "solid-js";

// ADR 1197 (the owner's addendum to ADR 0973's ruling 5): a read past an await in an async function every call of which is enumerated
// (same file, unexported, only direct calls) and runs from an `on…` handler, a component body or a helper that is so in turn: no async
// computation runs it, so no flight is on its continuation's async stack, and rc.21 raises nothing.

// Clean: called from the component body.
export function BodyCall() {
  const [s] = createSignal(0);
  const load = async () => {
    await Promise.resolve();
    s();
  };
  load();
  return <p />;
}

// Clean: called from an intrinsic element's handler.
export function HandlerCall() {
  const [s] = createSignal(0);
  const save = async () => {
    await Promise.resolve();
    s();
  };
  return <button onClick={() => save()}>go</button>;
}

// Clean: called from a helper the handler calls.
export function HelperChain() {
  const [s] = createSignal(0);
  const refresh = async () => {
    await Promise.resolve();
    s();
  };
  const go = () => {
    refresh();
  };
  return <button onClick={() => go()}>go</button>;
}

// Uncertifiable: exported, so unseen code may call it from a computation.
export async function exportedReader(read: () => number) {
  await Promise.resolve();
  return read();
}
export function ExportedCall() {
  const [s] = createSignal(0);
  exportedReader(s);
  return <p />;
}

// Uncertifiable: handed as a callback.
export function PassedAsCallback() {
  const [s] = createSignal(0);
  const later = async () => {
    await Promise.resolve();
    s();
  };
  queueMicrotask(later);
  return <p />;
}

// Uncertifiable: called from a memo's compute.
export function CalledFromMemo() {
  const [s] = createSignal(0);
  const fetchIt = async () => {
    await Promise.resolve();
    s();
  };
  const m = createMemo(() => {
    fetchIt();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}

// Uncertifiable: called from a helper a memo calls.
export function HelperFromMemo() {
  const [s] = createSignal(0);
  const fetchIt = async () => {
    await Promise.resolve();
    s();
  };
  const kick = () => {
    fetchIt();
  };
  const m = createMemo(() => {
    kick();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <p />;
}
