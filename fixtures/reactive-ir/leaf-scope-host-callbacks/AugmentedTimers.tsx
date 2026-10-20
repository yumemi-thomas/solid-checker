import { createSignal, createTrackedEffect, onCleanup, onSettled } from "solid-js";

declare const unknownTimer: typeof setInterval;

export function NativeTimers() {
  const [count] = createSignal(0);
  onSettled(() => {
    setInterval(() => { void count(); }, 100);
    setTimeout(() => { void count(); }, 100);
    queueMicrotask(() => { void count(); });
    (setTimeout as typeof setTimeout)(() => { void count(); }, 100);
  });
  return <div />;
}

export function OpaqueTimer() {
  const [count] = createSignal(0);
  onSettled(() => {
    unknownTimer(() => { void count(); }, 100);
  });
  return <div />;
}

export function ShadowedTimer() {
  function setInterval(_callback: () => void, _ms: number) {
    onCleanup(() => {});
    return 0;
  }
  createTrackedEffect(() => {
    setInterval(() => {}, 100);
  });
  return <div />;
}
