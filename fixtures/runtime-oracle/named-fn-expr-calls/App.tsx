import { createEffect, createMemo, createRenderEffect, createSignal, onSettled } from "solid-js";

export function ArrowPlain() {
  const [s] = createSignal(1);
  const cb = () => s();
  cb();
  return <div />;
}

export function NamedPlain() {
  const [s] = createSignal(1);
  const cb = function inner() { return s(); };
  cb();
  return <div />;
}

export function NamedSameName() {
  const [s] = createSignal(1);
  const cb = function cb() { return s(); };
  cb();
  return <div />;
}

export function NamedLet() {
  const [s] = createSignal(1);
  let cb = function inner() { return s(); };
  cb();
  return <div />;
}

export function NamedMemo() {
  const [s] = createSignal(1);
  const cb = function inner() { return s(); };
  const m = createMemo(cb);
  cb();
  return <div>{m()}</div>;
}

export function NamedSettled() {
  const [s] = createSignal(1);
  const cb = function inner() { s(); };
  onSettled(cb);
  cb();
  return <div />;
}

export function NamedEffect() {
  const [s] = createSignal(1);
  const cb = function inner() { return s(); };
  createEffect(cb, () => {});
  cb();
  return <div />;
}

export function NamedRenderEffect() {
  const [s] = createSignal(1);
  const cb = function inner() { return s(); };
  createRenderEffect(cb, () => {});
  cb();
  return <div />;
}

export function NamedHelper() {
  const [s] = createSignal(1);
  const cb = function inner() { return s(); };
  const m = createMemo(cb);
  const h = () => { cb(); };
  h();
  return <div>{m()}</div>;
}

export function NamedRecursive() {
  const [s] = createSignal(1);
  const cb = function inner(n: number): number { return n > 0 ? inner(n - 1) : s(); };
  cb(1);
  return <div />;
}

export function NamedHandler() {
  const [s] = createSignal(1);
  const cb = function inner() { return s(); };
  const m = createMemo(cb);
  return <button onClick={() => cb()}>{m()}</button>;
}

export function NamedMemoOnly() {
  const [s] = createSignal(1);
  const cb = function inner() { return s(); };
  const m = createMemo(cb);
  return <div>{m()}</div>;
}

export function NamedUncalled() {
  const [s] = createSignal(1);
  const cb = function inner() { return s(); };
  return <button onClick={cb} />;
}
