import * as S from "solid-js";
import { createEffect, createMemo, createSignal, createTrackedEffect, flush, onCleanup, onSettled } from "solid-js";

// ADR 1155 follow-up: a call the leaf refuses, written directly in a literal ADR 0200's proof forwards only into a leaf callback, runs on
// that callback's stack: SC3001 reports it where it is written, as if written at the wrapper's call of its parameter.

function hand(fn: () => void) {
  createTrackedEffect(() => {
    fn();
  });
}

function handNamespace(fn: () => void) {
  S.createTrackedEffect(() => {
    fn();
  });
}

// Violations.
export function SyncApp() {
  hand(() => {
    createEffect(async () => 1, () => {}, { sync: true });
  });
  return <i />;
}

export function MemoApp() {
  hand(() => {
    createMemo(() => 1);
  });
  return <i />;
}

export function CleanupApp() {
  hand(() => {
    onCleanup(() => {});
  });
  return <i />;
}

export function FlushApp() {
  hand(() => {
    flush();
  });
  return <i />;
}

// The namespace import's leaf and creation.
export function NamespaceApp() {
  handNamespace(() => {
    S.createMemo(() => 1);
  });
  return <i />;
}

// Clean: value-form state attaches nothing; the leaf allows it.
export function SignalApp() {
  hand(() => {
    createSignal(0);
  });
  return <i />;
}

// Clean: a local function that shadows the primitive's name.
export function ShadowedApp() {
  const createMemo = (fn: () => number) => fn();
  hand(() => {
    createMemo(() => 1);
  });
  return <i />;
}

// Clean: forwarded to a memo's compute, which owns children.
function handMemo(fn: () => void) {
  createMemo(() => {
    fn();
    return 0;
  });
}
export function MemoComputeApp() {
  handMemo(() => {
    createMemo(() => 1);
  });
  return <i />;
}

// Not a proven run in the leaf: written in a nested function the literal only creates.
export function NestedApp() {
  hand(() => {
    const later = () => createMemo(() => 1);
    void later;
  });
  return <i />;
}

// Not reported here: the literal is async (past its await it runs off the leaf's stack).
export function AsyncLiteralApp() {
  hand(async () => {
    await Promise.resolve();
    createMemo(() => 1);
  });
  return <i />;
}

// Not reported here: the wrapper also runs its argument on its own stack (`arguments[0]()`), outside the leaf.
function handBoth(fn: () => void) {
  arguments[0]();
  createTrackedEffect(() => {
    fn();
  });
}
export function BothApp() {
  handBoth(() => {
    createMemo(() => 1);
  });
  return <i />;
}

// Not reported here: onSettled is a leaf only under an owner its call site has, which the literal's file does not gate.
function handSettled(fn: () => void) {
  onSettled(() => {
    fn();
  });
}
export function SettledApp() {
  handSettled(() => {
    createMemo(() => 1);
  });
  return <i />;
}

// Not reported here: the literal reaches the leaf through a second wrapper, which hands it on by name.
function relay(fn: () => void) {
  hand(fn);
}
export function RelayApp() {
  relay(() => {
    createMemo(() => 1);
  });
  return <i />;
}

// Member wrapper: the same function through an object's method.
const api = { hand };
export function MemberApp() {
  api.hand(() => {
    createMemo(() => 1);
  });
  return <i />;
}
