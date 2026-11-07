import { action, createMemo, createRoot, createSignal, flush, onCleanup } from "solid-js";

const own: any = {};
own.forEach = (cb: () => void) => { queueMicrotask(cb); };
(globalThis as any).names = own;
const names: string[] = (globalThis as any).names;

export function HelperRead() {
  const [r] = createSignal(0);
  function h() { names.forEach(() => { r(); }); }
  h();
  return <p />;
}

export const step = action(function* () {
  names.forEach(() => { flush(); });
  yield;
});
const list = [1];
export const provenStep = action(function* () {
  list.forEach(() => { flush(); });
  yield;
});

export function ActionFlush() {
  return <button onClick={() => { step(); provenStep(); }} />;
}

const cleaned: any = {};
cleaned.forEach = (cb: () => void) => { onCleanup(cb); };
const later: string[] = cleaned;

export function RootWrite() {
  const [, set] = createSignal(0);
  later.forEach(() => { createRoot(() => set(1)); });
  return <p />;
}

const xs = [1, 2];
const ys: any = xs.reverse();
ys.forEach = own.forEach;

export function Answered() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => { xs.forEach(() => { set(1); }); return s(); });
  return <p>{m()}</p>;
}

const kept = [1, 2];
kept.reverse();

export function Discarded() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => { kept.forEach(() => { set(1); }); return s(); });
  return <p>{m()}</p>;
}
