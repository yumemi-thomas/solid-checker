import { createEffect, createMemo, createTrackedEffect } from "solid-js";

// ADR 1155 r3: a read in a literal ADR 0200's proof forwards to a computation's slot runs in that slot's callback, not in a render.
// It takes the role, the computation and the leaf owner of a read written at the wrapper's call of its parameter there.

function hand(fn: () => void) {
  createTrackedEffect(() => {
    fn();
  });
}

function handMemo(fn: () => void) {
  createMemo(() => {
    fn();
    return 0;
  });
}

// Forwarded to a memo's compute: a pending read suspends the memo; no render reads it, so no Loading boundary is asked
// (main: SC5003 a violation; Chrome quiet).
export function MemoApp() {
  const a1 = createMemo(async () => 1);
  handMemo(() => {
    a1();
  });
  return <i />;
}

// Forwarded to createTrackedEffect's compute: a pending read in a leaf throws (SC5001, Chrome PENDING_ASYNC_FORBIDDEN_SCOPE), as written
// there (main: SC5003 a violation, the wrong code).
export function TrackedApp() {
  const a1 = createMemo(async () => 1);
  hand(() => {
    a1();
  });
  return <i />;
}

// A `sync: true` creation forwarded into the leaf: the leaf refuses it before the result probe (main: SC7002 a violation; Chrome
// PRIMITIVE_IN_FORBIDDEN_SCOPE): SC7002 uncertifiable, creation-throws-first.
export function SyncApp() {
  hand(() => {
    createEffect(async () => 1, () => {}, { sync: true });
  });
  return <i />;
}

// The same read written directly in each compute: the twins this placement mirrors.
export function DirectMemoApp() {
  const a1 = createMemo(async () => 1);
  createMemo(() => {
    a1();
    return 0;
  });
  return <i />;
}
export function DirectTrackedApp() {
  const a1 = createMemo(async () => 1);
  createTrackedEffect(() => {
    a1();
  });
  return <i />;
}
