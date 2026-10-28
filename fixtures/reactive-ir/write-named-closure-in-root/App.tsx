import { action, createEffect, createMemo, createRoot, createSignal, onCleanup } from "solid-js";

// Violations: the closure runs under the root where it is called; rc.17 throws
// ACTION_CALLED_IN_OWNED_SCOPE when the button disposes the host (fast path).

// A closure bound to a name in an inline root body in a cleanup.
export function CleanupRootNamedClosure() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });
  const dispose = createRoot((d) => { onCleanup(() => { createRoot(() => { const g = () => { a1(); }; g(); }); }); return d; });
  return <div>{w1()}<button onClick={() => { dispose(); }}>go</button></div>;
}

// The same in a root body named by its binding.
export function CleanupRootByNameNamedClosure() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });
  const dispose = createRoot((d) => { onCleanup(() => { const f1 = () => { const g = () => { a1(); }; g(); }; createRoot(f1); }); return d; });
  return <div>{w1()}<button onClick={() => { dispose(); }}>go</button></div>;
}

// A function declaration in the root body.
export function CleanupRootDeclaredClosure() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });
  const dispose = createRoot((d) => { onCleanup(() => { createRoot(() => { function g() { a1(); } g(); }); }); return d; });
  return <div>{w1()}<button onClick={() => { dispose(); }}>go</button></div>;
}

// The root call in a named function the cleanup calls.
export function CleanupRootByNameInNamedFunction() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });
  const dispose = createRoot((d) => { onCleanup(() => { const f1 = () => { const g = () => { a1(); }; g(); }; function e() { createRoot(f1); } e(); }); return d; });
  return <div>{w1()}<button onClick={() => { dispose(); }}>go</button></div>;
}

// An IIFE in the named root body; the root call in an IIFE.
export function CleanupRootByNameInIife() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });
  const dispose = createRoot((d) => { onCleanup(() => { const f1 = () => { (() => { a1(); })(); }; (() => { createRoot(f1); })(); }); return d; });
  return <div>{w1()}<button onClick={() => { dispose(); }}>go</button></div>;
}

// The same in a memo's cleanup.
export function MemoCleanupRootByNameInIife() {
  const [w1, setW1] = createSignal(0);
  const [t1, setT1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });
  const dispose = createRoot((d) => { createMemo(() => { onCleanup(() => { const f1 = () => { (() => { a1(); })(); }; (() => { createRoot(f1); })(); }); return t1(); }); return d; });
  return <div>{w1()}<button onClick={() => { dispose(); }}>go</button></div>;
}

// An effect's returned cleanup at teardown.
export function EffectCleanupRootByName() {
  const [w1, setW1] = createSignal(0);
  const [t1, setT1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });
  const dispose = createRoot((d) => { createEffect(() => t1(), () => { return () => { const f1 = () => { const g = () => { a1(); }; g(); }; createRoot(f1); }; }); return d; });
  return <div>{w1()}<button onClick={() => { dispose(); }}>go</button></div>;
}

// A root body in a handler.
export function HandlerRootNamedClosure() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });

  return <div>{w1()}<button onClick={() => { createRoot(() => { const g = () => { a1(); }; g(); }); }}>go</button></div>;
}

// Called in place and also kept: the run in place is enough.
export function CleanupRootNamedClosureAlsoKept() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });
  let keep: (() => void) | undefined; const dispose = createRoot((d) => { onCleanup(() => { createRoot(() => { const g = () => { a1(); }; g(); keep = g; }); }); return d; });
  return <div>{w1()}<button onClick={() => { dispose(); }}>go</button></div>;
}

// Legal (rc.17 silent).

// A signal write: the guard stands down in the disposal.
export function CleanupRootWriteNamedClosure() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });
  const dispose = createRoot((d) => { onCleanup(() => { createRoot(() => { const g = () => { setW1(3); }; g(); }); }); return d; });
  return <div>{w1()}<button onClick={() => { dispose(); }}>go</button></div>;
}

// The same through an IIFE in a named root body.
export function CleanupRootByNameWriteInIife() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });
  const dispose = createRoot((d) => { onCleanup(() => { const f1 = () => { (() => { setW1(3); })(); }; (() => { createRoot(f1); })(); }); return d; });
  return <div>{w1()}<button onClick={() => { dispose(); }}>go</button></div>;
}

// The closure handed to a timer runs with no owner.
export function CleanupRootClosureOnTimer() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });
  const dispose = createRoot((d) => { onCleanup(() => { createRoot(() => { const g = () => { a1(); }; setTimeout(g); }); }); return d; });
  return <div>{w1()}<button onClick={() => { dispose(); }}>go</button></div>;
}

// A closure in a handler with no root.
export function HandlerNamedClosure() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });

  return <div>{w1()}<button onClick={() => { const g = () => { a1(); }; g(); }}>go</button></div>;
}

// A local createRoot that schedules its callback.
export function ShadowedCreateRoot() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });
  const dispose = createRoot((d) => { onCleanup(() => { const createRoot = (fn: () => void) => { setTimeout(fn); }; createRoot(() => { const g = () => { a1(); }; g(); }); }); return d; });
  return <div>{w1()}<button onClick={() => { dispose(); }}>go</button></div>;
}

// The action closure of the body shares its name with a root body's legal one.
export function ReusedClosureName() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });
  const g = () => { a1(); }; const dispose = createRoot((d) => { onCleanup(() => { createRoot(() => { const g = () => { setW1(2); }; g(); }); }); return d; });
  return <div>{w1()}<button onClick={() => { g(); dispose(); }}>go</button></div>;
}

// Uncertifiable: legal at run time, but the closure runs where its holder calls it.

// Stored and called from the handler.
export function CleanupRootClosureStored() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });
  let keep: (() => void) | undefined; const dispose = createRoot((d) => { onCleanup(() => { createRoot(() => { const g = () => { a1(); }; keep = g; }); }); return d; });
  return <div>{w1()}<button onClick={() => { dispose(); keep?.(); }}>go</button></div>;
}

// Returned from the root and run on a timer.
export function CleanupRootClosureReturned() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });
  const dispose = createRoot((d) => { onCleanup(() => { const h = createRoot(() => { const g = () => { a1(); }; return g; }); setTimeout(h); }); return d; });
  return <div>{w1()}<button onClick={() => { dispose(); }}>go</button></div>;
}

// Violation (rc.17 throws): `run` has one call, after both declarators, so the
// root call in it runs there and `f1` is initialized (ADR 0479).

// The root call in a closure declared before the root body.
export function CleanupRootByNameInNestedFunction() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(17); });
  const dispose = createRoot((d) => { onCleanup(() => { const run = () => { createRoot(f1); }; const f1 = () => { (() => { a1(); })(); }; run(); }); return d; });
  return <div>{w1()}<button onClick={() => { dispose(); }}>go</button></div>;
}
