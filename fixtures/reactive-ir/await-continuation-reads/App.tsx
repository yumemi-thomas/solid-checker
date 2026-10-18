import { createSignal, createStore } from "solid-js";

// ADR 0323 (review round 9): a read after an await its async function reaches
// on every path runs from a promise continuation on a later task. The
// strict-read window is the synchronous extent of `untrack(fn, label)`
// (`@solidjs/signals@2.0.0-rc.13` `dist/dev-shared.js:5801-5885`: `strictRead`
// is restored in the `finally`), so there is no window and no tracking scope
// for it. A read before the first await is in the window; an await that may
// be skipped (`x ??= await p`, an optional-call argument, a labelled break)
// does not make what follows a continuation.

function now(f: () => void) {
  f();
}

declare const p: Promise<number>;
declare const maybe: number | undefined;

// Signals.
export function BeforeAwait() {
  const [count] = createSignal(0);
  now(async () => {
    console.log(count());
    await 0;
  });
  return <div />;
}
export function AfterAwait() {
  const [count] = createSignal(0);
  now(async () => {
    await 0;
    console.log(count());
  });
  return <div />;
}
// A skippable await: the read may still run in the window.
export function SkippableAssign() {
  const [count] = createSignal(0);
  now(async () => {
    let x = maybe;
    x ??= await p;
    console.log(count(), x);
  });
  return <div />;
}
export function SkippableOptionalCall() {
  const [count] = createSignal(0);
  const f = maybe === undefined ? undefined : (n: number) => n;
  now(async () => {
    f?.(await p);
    console.log(count());
  });
  return <div />;
}
export function SkippableLabelled() {
  const [count] = createSignal(0);
  now(async () => {
    skip: {
      if (maybe === undefined) break skip;
      await p;
    }
    console.log(count());
  });
  return <div />;
}
// An async IIFE called in the body: the part before its await is in the window.
export function AsyncIife() {
  const [count] = createSignal(0);
  (async () => {
    console.log(count());
    await 0;
    console.log(count() + 1);
  })();
  return <div />;
}
// Store and props, same shapes.
export function StoreAfterAwait() {
  const [s] = createStore({ a: 1 });
  now(async () => {
    await 0;
    console.log(s.a);
  });
  return <div />;
}
export function StoreBeforeAwait() {
  const [s] = createStore({ a: 1 });
  now(async () => {
    console.log(s.a);
    await 0;
  });
  return <div />;
}
export function PropsAfterAwait(props: { x: number }) {
  now(async () => {
    await 0;
    console.log(props.x);
  });
  return <div />;
}
export function PropsBeforeAwait(props: { x: number }) {
  now(async () => {
    console.log(props.x);
    await 0;
  });
  return <div />;
}
