import { createEffect, createSignal, createStore } from "solid-js";

const [st, setSt] = createStore({ n: 0 });

// ADR 0833: a read in a store setter's callback inside a named helper an
// effect's apply calls runs untracked on the apply's stack (Chrome, rc.21:
// STRICT_READ_UNTRACKED). No summary carries a read in the setter's callback,
// so it is an SC1001 obligation, `function-run-unplaced`, at the read.
export function InHelper() {
  const [a] = createSignal(0);
  const wrapUse = () => { setSt((d) => { d.n = a(); }); };
  createEffect(() => 1, () => { wrapUse(); });
  return <i />;
}

// Kept: the same read with the setter called in the apply itself is a
// violation, placed by the setter's call (ADR 0676).
export function InApply() {
  const [a] = createSignal(0);
  createEffect(() => 1, () => { setSt((d) => { d.n = a(); }); });
  return <i />;
}

// Silent: the setter's own draft is read, whose Gets never warn.
export function DraftInHelper() {
  const wrapUse = () => { setSt((d) => { d.n = d.n + 1; }); };
  createEffect(() => 1, () => { wrapUse(); });
  return <i />;
}

void st;
