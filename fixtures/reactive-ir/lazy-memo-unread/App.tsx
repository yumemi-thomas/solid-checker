import { action, createContext, createMemo, createSignal, useContext } from "solid-js";
import * as S from "solid-js";

// Default-less and never provided: a read that runs under an owner throws ContextNotFoundError.
const Ctx = createContext<number>();
declare const flag: boolean;
declare function keep(accessor: () => number): void;

// Obligations (ADR 0576): a lazy memo nothing reads never computes, so nothing in it is proven.
export function UnreadWrite() {
  const [w, setW] = createSignal(0);
  const m = createMemo(() => {
    setW(7);
    return 1;
  }, { lazy: true });
  return <div>{w()}</div>;
}

export function UnreadContext() {
  const m = createMemo(() => {
    useContext(Ctx);
    return 1;
  }, { lazy: true });
  return <div></div>;
}

export function UnreadAction() {
  const [w, setW] = createSignal(0);
  const a = action(function* () {
    setW(1);
  });
  const m = createMemo(() => {
    a();
    return 1;
  }, { lazy: true });
  return <div>{w()}</div>;
}

// A helper only the lazy compute calls runs only when it does.
export function UnreadHelper() {
  const [w, setW] = createSignal(0);
  function write() {
    setW(7);
  }
  const m = createMemo(() => {
    write();
    return 1;
  }, { lazy: true });
  return <div>{w()}</div>;
}

// A named compute handed by reference.
export function UnreadNamedCompute() {
  const [w, setW] = createSignal(0);
  const compute = () => {
    setW(7);
    return 1;
  };
  const m = createMemo(compute, { lazy: true });
  return <div>{w()}</div>;
}

// Through the namespace.
export function UnreadNamespace() {
  const [w, setW] = S.createSignal(0);
  const m = S.createMemo(() => {
    setW(7);
    return 1;
  }, { lazy: true });
  return <div>{w()}</div>;
}

// A function-form signal takes the same option.
export function UnreadSignalForm() {
  const [w, setW] = createSignal(0);
  const [m] = createSignal(() => {
    setW(7);
    return 1;
  }, { lazy: true });
  return <div>{w()}</div>;
}

// An option the syntax does not decide may be lazy.
export function UnknownLazy() {
  const [w, setW] = createSignal(0);
  const m = createMemo(() => {
    setW(7);
    return 1;
  }, { lazy: flag });
  return <div>{w()}</div>;
}

// A handed accessor proves no read: whoever holds it may read it, but no direct call does.
export function HandedAccessor() {
  const [w, setW] = createSignal(0);
  const m = createMemo(() => {
    setW(7);
    return 1;
  }, { lazy: true });
  keep(m);
  return <div>{w()}</div>;
}

// Violations (no-write-in-pure-computation, missing-context-provider): the compute runs.
export function NotLazy() {
  const [w, setW] = createSignal(0);
  const m = createMemo(() => {
    setW(7);
    return 1;
  });
  return <div>{w()}</div>;
}

export function LazyFalse() {
  const [w, setW] = createSignal(0);
  const m = createMemo(() => {
    setW(7);
    return 1;
  }, { lazy: false });
  return <div>{w()}</div>;
}

export function ReadInBody() {
  const m = createMemo(() => {
    useContext(Ctx);
    return 1;
  }, { lazy: true });
  return <div>{m()}</div>;
}

export function ReadInHandler() {
  const [w, setW] = createSignal(0);
  const m = createMemo(() => {
    setW(7);
    return 1;
  }, { lazy: true });
  return <button onClick={() => m()}>{w()}</button>;
}

export function ReadSignalForm() {
  const [w, setW] = createSignal(0);
  const [m] = createSignal(() => {
    setW(7);
    return 1;
  }, { lazy: true });
  return <div>{m()}</div>;
}
