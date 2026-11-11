import { createMemo, createSignal, createTrackedEffect, flush, isPending } from "solid-js";

const [n] = createSignal(0);

function make(skip: boolean) {
  if (skip) return;
  createMemo(() => 1);
}
function runTwice() {
  make(true);
  make(false);
}
function runSkipped() {
  make(true);
  make(true);
}
function flushes() {
  flush();
}
function pendingThenDirect() {
  isPending(flushes);
  flushes();
}
function pendingOnly() {
  isPending(flushes);
}
function recurse(k: number): void {
  if (k > 0) recurse(k - 1);
}

// A function entered twice under different contexts: the second entry is the forbidden one (ADR 0961).
export function TwiceForbidden() {
  createTrackedEffect(() => {
    n();
    runTwice();
  });
  return <p>x</p>;
}
export function SwallowedThenDirect() {
  createTrackedEffect(() => {
    n();
    pendingThenDirect();
  });
  return <p>x</p>;
}
// The same functions entered only the tolerant way, or recursing: clean.
export function TwiceSkipped() {
  createTrackedEffect(() => {
    n();
    runSkipped();
  });
  return <p>x</p>;
}
export function SwallowedOnly() {
  createTrackedEffect(() => {
    n();
    pendingOnly();
  });
  return <p>x</p>;
}
export function Recursion() {
  createTrackedEffect(() => {
    n();
    recurse(3);
  });
  return <p>x</p>;
}
