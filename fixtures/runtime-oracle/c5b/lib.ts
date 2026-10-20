import { createMemo, createTrackedEffect, onCleanup, onSettled, untrack, createEffect } from "solid-js";

export function retain(cb: () => void) {
  return () => cb();
}
export function handlers(cb: () => void) {
  return { onKey: () => cb(), onClick: () => cb() };
}
export function loopy(cb: () => void) {
  for (let i = 0; i < 1; i++) cb();
  return { onClick: () => cb() };
}
export function viaHelper(cb: () => void) {
  const helper = () => cb();
  helper();
  return { onClick: () => cb() };
}
export function viaTry(cb: () => void) {
  try {
    return { onClick: () => cb() };
  } finally {
    cb();
  }
}
export function inMemo(cb: () => void) {
  const m = createMemo(() => (cb(), 1));
  return m;
}
export function inCleanup(cb: () => void) {
  onCleanup(() => cb());
}
export function inSettled(cb: () => void) {
  onSettled(() => cb());
}
export function inTracked(cb: () => void) {
  createTrackedEffect(() => cb());
}
export function inApply(cb: () => void) {
  createEffect(() => 1, () => cb());
}
export function inUntrack(cb: () => void) {
  return untrack(() => {
    cb();
    return 1;
  });
}
export function iife(cb: () => void) {
  (() => cb())();
  return { onClick: () => cb() };
}
export function viaObjectCall(cb: () => void) {
  const o = { run: () => cb() };
  o.run();
  return { onClick: () => cb() };
}
export function viaTimeoutLater(cb: () => void) {
  setTimeout(() => cb(), 0);
}
export function sameTick(cb: () => void) {
  Promise.resolve().then(() => cb());
}
