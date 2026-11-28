import { createEffect, createMemo, createSignal } from "solid-js";

// ADR 1235: a function declared in an effect's apply and called there runs its body on that call's stack: the read its returned
// value runs (a method of the returned object, a returned arrow, an element of a returned array) is an untracked read at the call
// (rc.21 STRICT_READ_UNTRACKED on the grid's `handed-*` effect-apply programs).
export function Chain() {
  const [a1] = createSignal(0);
  createEffect(
    () => 1,
    () => {
      const useT = (n: () => number) => ({ read: (): number => n() });
      void useT(a1).read();
    },
  );
  return <p>x</p>;
}

export function Curried() {
  const [a1] = createSignal(0);
  createEffect(
    () => 1,
    () => {
      const useT = (n: () => number) => () => () => n();
      void useT(a1)()();
    },
  );
  return <p>x</p>;
}

export function Element() {
  const [a1] = createSignal(0);
  createEffect(
    () => 1,
    () => {
      const useT = (n: () => number) => [(): number => n()];
      useT(a1).forEach((f) => void f());
    },
  );
  return <p>x</p>;
}

export function HeldElement() {
  const [a1] = createSignal(0);
  createEffect(
    () => 1,
    () => {
      const useT = (n: () => number) => [(): number => n()];
      const x = useT(a1);
      x.forEach((f) => void f());
    },
  );
  return <p>x</p>;
}

export function ElementMember() {
  const [a1] = createSignal(0);
  createEffect(
    () => 1,
    () => {
      const useT = (n: () => number) => [{ m: (): number => n() }];
      const x = useT(a1);
      x.forEach((e) => e.m());
    },
  );
  return <p>x</p>;
}

// Uncertifiable: the held array has a second reference, so a holder could have replaced an element (ADR 0282); no element is run.
export function HeldTwice() {
  const [a1] = createSignal(0);
  createEffect(
    () => 1,
    () => {
      const useT = (n: () => number) => [(): number => n()];
      const x = useT(a1);
      void x.length;
      x.forEach((f) => void f());
    },
  );
  return <p>x</p>;
}

// Not a violation: the same chain in a memo's compute, which tracks the read (uncertifiable today: the dispatch obligation stays).
export function InMemo() {
  const [a1] = createSignal(0);
  const m = createMemo(() => {
    const useT = (n: () => number) => ({ read: (): number => n() });
    return useT(a1).read();
  });
  return <p>{m()}</p>;
}

// Clean: the same chain in a click handler, where no window is open.
export function InHandler() {
  const [a1] = createSignal(0);
  return (
    <button
      onClick={() => {
        const useT = (n: () => number) => ({ read: (): number => n() });
        void useT(a1).read();
      }}
    />
  );
}

// Not a violation: the function is declared in the apply but only handed on to a later run (`queueMicrotask`), so no read runs there
// (uncertifiable: the microtask's run is unplaced).
export function Deferred() {
  const [a1] = createSignal(0);
  createEffect(
    () => 1,
    () => {
      const useT = () => ({ read: (): number => a1() });
      queueMicrotask(() => void useT().read());
    },
  );
  return <p>x</p>;
}
