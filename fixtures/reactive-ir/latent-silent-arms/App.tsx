import { createEffect, createMemo, createSignal, For, onSettled, Show, untrack } from "solid-js";

// ADR 0927 (rows 1-4 of the silence audit; each positive case warns
// STRICT_READ_UNTRACKED in Chrome on rc.21).

// Uncertifiable: a closed untrack keeps the function in a Set, the body runs it.
export function KeptInSet() {
  const [count] = createSignal(3);
  const kept = new Set<() => void>();
  untrack(() => {
    const f = () => { count(); };
    kept.add(f);
  });
  kept.forEach((g) => g());
  return <div>x</div>;
}

// Uncertifiable: the same through an array's push.
export function KeptInArray() {
  const [count] = createSignal(3);
  const kept: Array<() => void> = [];
  untrack(() => {
    const f = () => { count(); };
    kept.push(f);
  });
  for (const g of kept) g();
  return <div>x</div>;
}

// Uncertifiable: the same through a Map's set.
export function KeptInMap() {
  const [count] = createSignal(3);
  const kept = new Map<number, () => void>();
  untrack(() => {
    const f = () => { count(); };
    kept.set(1, f);
  });
  kept.forEach((g) => g());
  return <div>x</div>;
}

// Silent: a listener the host keeps, in a program that dispatches nothing.
export function ListenerInSettled() {
  const [count] = createSignal(3);
  onSettled(() => {
    const f = () => { count(); };
    document.body.addEventListener("click", f);
  });
  return <div>x</div>;
}

// Violation: a For row created in a closed untrack reads in the row's window.
export function ForInClosedUntrack() {
  const [count] = createSignal(3);
  const el = untrack(() => <For each={[1]}>{() => { count(); return <li>x</li>; }}</For>, false);
  return <div>{el}</div>;
}

// Violation: a one-parameter Show child created in a closed untrack.
export function ShowInClosedUntrack() {
  const [count] = createSignal(3);
  const el = untrack(() => <Show when={1}>{(v: () => number) => { void v; count(); return <p>x</p>; }}</Show>);
  return <div>{el}</div>;
}

// Silent: a row's click handler, created in a closed untrack.
export function HandlerInRowInClosedUntrack() {
  const [count] = createSignal(3);
  const el = untrack(() => <For each={[1]}>{() => <button onClick={() => { count(); }}>x</button>}</For>);
  return <div>{el}</div>;
}

// Silent: a memo's compute, created in a closed untrack, tracks its read.
export function MemoInClosedUntrack() {
  const [count] = createSignal(3);
  const m = untrack(() => createMemo(() => count()));
  return <div>{m()}</div>;
}

// Uncertifiable: the apply calls the function the compute returns, inside void.
export function VoidCalledApply() {
  const [count] = createSignal(3);
  untrack(() => {
    createEffect(() => () => { count(); }, (v) => { void v(); });
  });
  return <div>x</div>;
}

// Silent: the apply only names it, bare or parenthesized.
export function VoidNamedApply() {
  const [count] = createSignal(3);
  untrack(() => {
    createEffect(() => () => { count(); }, (v) => { void v; });
    createEffect(() => () => { count(); }, (v) => { void (v); });
  });
  return <div>x</div>;
}

// Uncertifiable: a `function` apply has an `arguments` object; nothing proves it idle.
export function VoidNamedFunctionApply() {
  const [count] = createSignal(3);
  untrack(() => {
    createEffect(() => () => { count(); }, function (v) { void v; });
  });
  return <div>x</div>;
}
