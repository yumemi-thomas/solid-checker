import { action, createEffect, createSignal, createTrackedEffect, flush } from "solid-js";

const [n] = createSignal(0);

// ADR 1199: the program steps an action, runs an effect's apply and opens a leaf scope, so no program-wide reason holds and each flush stands on where it is
// placed. A flush with no census class of its own, written in a function whose every run is a plain call (or, for a component prop's literal, the child's
// calls of the prop), is asked at each of those calls: when every one has a class that rules every code out, the flush does too.
export const step = action(function* () {
  yield 1;
});

export function Host() {
  createEffect(() => n(), () => {});
  createTrackedEffect(() => {
    n();
  });
  return <div />;
}

// Clean: a helper called from two timer callbacks (no single-call layer, so ADR 1090's climb stops before it).
function twice() {
  flush();
}
export function Twice() {
  setTimeout(() => {
    twice();
  }, 0);
  setTimeout(() => {
    twice();
  }, 10);
  return <div />;
}

// Clean: a helper called from a timer callback and after an await.
function timerAndAwait() {
  flush();
}
export function TimerAndAwait() {
  setTimeout(() => {
    timerAndAwait();
  }, 0);
  void (async () => {
    await Promise.resolve();
    timerAndAwait();
  })();
  return <div />;
}

// Clean: a helper whose only caller is a helper called from two timer callbacks (the entries of the entries).
function inner() {
  flush();
}
function outer() {
  inner();
  inner();
}
export function Chain() {
  setTimeout(() => {
    outer();
  }, 0);
  setTimeout(() => {
    outer();
  }, 10);
  return <div />;
}

// Clean: a component prop's literal the child calls only in timer callbacks.
function Later(props: { run: () => void }) {
  setTimeout(() => {
    props.run();
  }, 0);
  setTimeout(() => {
    props.run();
  }, 10);
  return <p />;
}
export function PropLater() {
  return <Later run={() => flush()} />;
}

// Clean: an IIFE written after an await runs at its call, on the continuation.
export function AfterAwaitIife() {
  void (async () => {
    await Promise.resolve();
    (() => {
      flush();
    })();
  })();
  return <div />;
}

// Uncertifiable (fail closed): the same IIFE before the await runs on the stack of the component body, which has no class.
export function BeforeAwaitIife() {
  void (async () => {
    (() => {
      flush();
    })();
    await Promise.resolve();
  })();
  return <div />;
}

// Uncertifiable (fail closed): the helper is also called from an effect's apply.
function inApply() {
  flush();
}
export function InApply() {
  setTimeout(() => {
    inApply();
  }, 0);
  createEffect(
    () => n(),
    () => {
      inApply();
    },
  );
  return <div />;
}

// Uncertifiable (fail closed): the helper is also called from an event handler (a delivery may be dispatched from a step: owner ruling 5a).
function inHandler() {
  flush();
}
export function InHandler() {
  setTimeout(() => {
    inHandler();
  }, 0);
  return <button onClick={() => inHandler()}>go</button>;
}

// Uncertifiable (fail closed): the helper is also stored in an object and called through the member.
function viaMember() {
  flush();
}
const holder = { m: viaMember };
export function ViaMember() {
  setTimeout(() => {
    viaMember();
  }, 0);
  setTimeout(() => {
    holder.m();
  }, 10);
  return <div />;
}

// Uncertifiable (fail closed): the child calls the prop's literal in its body too (a component body has no class).
function Now(props: { run: () => void }) {
  setTimeout(() => {
    props.run();
  }, 0);
  props.run();
  return <p />;
}
export function PropNow() {
  return <Now run={() => flush()} />;
}
