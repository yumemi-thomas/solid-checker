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

// The two entries written in a closure the host's callback declares and calls (the walk reads the closure's body, then each call in it).
export function TwiceInCallback() {
  createTrackedEffect(() => {
    n();
    const both = () => {
      make(true);
      make(false);
    };
    both();
  });
  return <p>x</p>;
}
export function SwallowedThenDirectInCallback() {
  createTrackedEffect(() => {
    n();
    const flushesHere = () => {
      flush();
    };
    isPending(flushesHere);
    flushesHere();
  });
  return <p>x</p>;
}
// An argument the walk cannot decide: exactly one of `make(x)` and `make(!x)` creates, whatever `x` is. Never clean.
function runUnknown(x: boolean) {
  make(x);
  make(!x);
}
export function TwiceUnknown() {
  createTrackedEffect(() => {
    runUnknown(n() > 0);
  });
  return <p>x</p>;
}
