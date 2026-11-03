import { createContext, createMemo, createSignal, useContext } from "solid-js";

// Default-less and never provided: a read that runs under an owner throws ContextNotFoundError.
const Ctx = createContext<number>();

// Uncertifiable (ADR 0578): each module-scope helper is unexported and named only from code that never runs.
function readInIf() { useContext(Ctx); }
function readAfterReturn() { useContext(Ctx); }
function readAfterFalse() { useContext(Ctx); }
const readArrow = () => { useContext(Ctx); };
function readInLazy() { useContext(Ctx); }
function Child1() {
  useContext(Ctx);
  return <p>never rendered</p>;
}
const [shared, setShared] = createSignal(0);
function writeInIf() { setShared(7); }

export function DeadIf() {
  if (false) {
    readInIf();
  }
  return <div></div>;
}

export function DeadAfterReturn() {
  return <div></div>;
  readAfterReturn();
}

export function DeadAfterFalse() {
  false && readAfterFalse();
  return <div></div>;
}

export function DeadArrowHelper() {
  if (false) readArrow();
  return <div></div>;
}

// ADR 0576 through a module helper: only an unread lazy memo calls it.
export function LazyThroughHelper() {
  const m = createMemo(() => {
    readInLazy();
    return 1;
  }, { lazy: true });
  return <div></div>;
}

// A write only a dead branch of a memo reaches (SC2001's site is the helper's).
export function DeadWriteHelper() {
  const m = createMemo(() => {
    if (false) writeInIf();
    return shared();
  });
  return <div>{m()}</div>;
}

// Violations: the same helpers reached.
function readLive() { useContext(Ctx); }
const readLiveArrow = () => { useContext(Ctx); };
function Child2() {
  useContext(Ctx);
  return <p>rendered</p>;
}
function writeLive() { setShared(7); }

export function LiveHelpers() {
  readLive();
  readLiveArrow();
  return <div><Child2 /></div>;
}

export function LiveWriteHelper() {
  const m = createMemo(() => {
    writeLive();
    return shared();
  });
  return <div>{m()}</div>;
}
