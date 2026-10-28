import { createEffect, createMemo, createRoot, createSignal, onCleanup, runWithOwner, untrack } from "solid-js";

// Uncertifiable where rc.17 reports NO_OWNER_CLEANUP / NO_OWNER_EFFECT (fast path):
// a returned function runs where its caller calls it, and no owner edge follows it.

// A plain factory's result called on a timer.
export function FactoryResultOnTimer() {
  const [w1, setW1] = createSignal(0);
  const mk = () => () => { onCleanup(() => {}); }; const h = mk(); setTimeout(() => h());
  return <div>{w1()}</div>;
}

// The same called from a handler.
export function FactoryResultInHandler() {
  const [w1, setW1] = createSignal(0);
  const mk = () => () => { onCleanup(() => {}); }; const h = mk();
  return <div>{w1()}<button onClick={() => { h(); }}>go</button></div>;
}

// A return statement in a function declaration.
export function FactoryReturnStatementOnTimer() {
  const [w1, setW1] = createSignal(0);
  function mk() { return () => { onCleanup(() => {}); }; } const h = mk(); setTimeout(() => h());
  return <div>{w1()}</div>;
}

// Returned through runWithOwner(null, ...).
export function NullOwnerResultOnTimer() {
  const [w1, setW1] = createSignal(0);
  const h = runWithOwner(null, () => () => { onCleanup(() => {}); }); setTimeout(() => h());
  return <div>{w1()}</div>;
}

// Returned through createRoot.
export function RootResultOnTimer() {
  const [w1, setW1] = createSignal(0);
  const h = createRoot(() => () => { onCleanup(() => {}); }); setTimeout(() => h());
  return <div>{w1()}</div>;
}

// Returned through untrack, called on a timer.
export function UntrackResultOnTimer() {
  const [w1, setW1] = createSignal(0);
  const h = untrack(() => () => { onCleanup(() => {}); }); setTimeout(() => h());
  return <div>{w1()}</div>;
}

// The same called from a handler.
export function UntrackResultInHandler() {
  const [w1, setW1] = createSignal(0);
  const h = untrack(() => () => { onCleanup(() => {}); });
  return <div>{w1()}<button onClick={() => { h(); }}>go</button></div>;
}

// A memo's value called on a timer.
export function MemoValueOnTimer() {
  const [w1, setW1] = createSignal(0);
  const m = createMemo(() => () => { onCleanup(() => {}); }); setTimeout(() => m()());
  return <div>{w1()}</div>;
}

// An effect in the returned function.
export function FactoryEffectOnTimer() {
  const [w1, setW1] = createSignal(0);
  const mk = () => () => { createEffect(() => w1(), () => {}); }; const h = mk(); setTimeout(() => h());
  return <div>{w1()}</div>;
}

// The cleanup in an IIFE in the returned function.
export function FactoryIifeOnTimer() {
  const [w1, setW1] = createSignal(0);
  const mk = () => () => { (() => { onCleanup(() => {}); })(); }; const h = mk(); setTimeout(() => h());
  return <div>{w1()}</div>;
}

// Two results of one factory: one called in the body, one on a timer.
export function FactoryCalledTwice() {
  const [w1, setW1] = createSignal(0);
  const mk = () => () => { onCleanup(() => {}); }; const h = mk(); const h2 = mk(); h2(); setTimeout(() => h());
  return <div>{w1()}</div>;
}

// Uncertifiable, legal at run time: the cost of the fallback.

// The result called in the body, under the component.
export function FactoryResultInBody() {
  const [w1, setW1] = createSignal(0);
  const mk = () => () => { onCleanup(() => {}); }; const h = mk(); h();
  return <div>{w1()}</div>;
}

// The result never called.
export function FactoryResultNeverCalled() {
  const [w1, setW1] = createSignal(0);
  const mk = () => () => { onCleanup(() => {}); }; const h = mk(); void h;
  return <div>{w1()}</div>;
}

// The result called in a root on a timer.
export function FactoryResultInRoot() {
  const [w1, setW1] = createSignal(0);
  const mk = () => () => { onCleanup(() => {}); }; const h = mk(); setTimeout(() => createRoot(() => h()));
  return <div>{w1()}</div>;
}

// Violation: a cleanup written directly in a timer callback (unchanged).

// rc.17 reports NO_OWNER_CLEANUP.
export function CleanupOnTimer() {
  const [w1, setW1] = createSignal(0);
  setTimeout(() => { onCleanup(() => {}); });
  return <div>{w1()}</div>;
}

// Legal (rc.17 silent), certified.

// A closure called in the body is not returned.
export function ClosureCalledInBody() {
  const [w1, setW1] = createSignal(0);
  const h = () => { onCleanup(() => {}); }; h();
  return <div>{w1()}</div>;
}

// A local onCleanup is no owner operation.
export function ShadowedOnCleanup() {
  const [w1, setW1] = createSignal(0);
  const mk = () => { const onCleanup = (fn: () => void) => fn; return () => { onCleanup(() => {}); }; }; const h = mk(); setTimeout(() => h());
  return <div>{w1()}</div>;
}
