import * as Solid from "solid-js";
import { createEffect, createMemo, createRoot, createSignal, untrack } from "solid-js";

const [signal] = createSignal(0);

// ADR 0932 (owner ruling, 2026-11-10): `createRoot(init)` is
// `runWithOwner(createOwner(), …)` at rc.21, so each case below reports, or
// stays clean, exactly as its `runWithOwner` twin in App.tsx and Aliases.tsx.

// Positive: an effect compute (App.tsx case 32).
createRoot(() => {
  createEffect(
    () => createRoot(() => console.log(signal())),
    () => {},
  );
});

// Positive: inside a component (case 33).
export function RootComponent() {
  const [local] = createSignal(0);
  createEffect(
    () => createRoot(() => console.log(local())),
    () => {},
  );
  return null;
}

// Positive: a memo compute.
export function RootDerived() {
  const value = createMemo(() => createRoot(() => signal()));
  return <span>{value()}</span>;
}

// Positive: a JSX expression.
export function RootInline() {
  return <span>{createRoot(() => signal())}</span>;
}

// Positive: the namespace spelling.
export function RootViaNamespace() {
  const value = createMemo(() => Solid.createRoot(() => signal()));
  return <span>{value()}</span>;
}

// Negative: the callback only creates a primitive.
export function RootCreates() {
  const value = createMemo(() => createRoot(() => createMemo(() => signal())));
  return <span>{value()()}</span>;
}

// Negative: an explicit one-time snapshot.
export function RootSnapshot() {
  const value = createMemo(() => createRoot(() => untrack(() => signal())));
  return <span>{value()}</span>;
}

// Negative: written directly in an `untrack` callback.
export function RootInUntrack() {
  const value = createMemo(() => untrack(() => createRoot(() => signal())));
  return <span>{value()}</span>;
}

// Negative: JSX the callback writes tracks its own expression.
export function RootRenders() {
  const view = createMemo(() => createRoot(() => <b>{signal()}</b>));
  return <div>{view()}</div>;
}

// Negative: after an await (SC1002's).
export function RootAfter() {
  const value = createMemo(async () => {
    await Promise.resolve();
    return createRoot(() => signal());
  });
  return <span>{String(value())}</span>;
}

// Negative: a local function of the same name is not Solid's.
function localRoot<T>(fn: () => T): T {
  return fn();
}

export function RootShadowed() {
  const createRoot = localRoot;
  const value = createMemo(() => createRoot(() => signal()));
  return <span>{value()}</span>;
}

// A project wrapper that forwards to the primitive: the literal is handed to
// project code, so the call does not place it.
function inRoot<T>(fn: () => T): T {
  return Solid.createRoot(fn);
}

export function RootWrapped() {
  const value = createMemo(() => inRoot(() => signal()));
  return <span>{value()}</span>;
}
