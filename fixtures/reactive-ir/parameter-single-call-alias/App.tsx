import { createEffect, createMemo, createSignal } from "solid-js";

// ADR 1233: a local function's parameter bound to the one argument of its one call holds exactly that accessor: the read in the
// method it returns, called at once in the component body, is an untracked read (rc.21 STRICT_READ_UNTRACKED).
export function HandedChain() {
  const [a1] = createSignal(0);
  const useT = (n: () => number) => ({ read: (): number => n() });
  void useT(a1).read();
  return <p>x</p>;
}

// Violation (ADR 1235): the same read in an effect's apply. The method the returned object holds runs on the call's stack, which
// the apply runs untracked.
export function EffectApplyChain() {
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

// Uncertifiable: the function has two references (two calls), so the parameter holds no one value.
export function TwoCalls() {
  const [a1] = createSignal(0);
  const [b1] = createSignal(1);
  const useT = (n: () => number) => ({ read: (): number => n() });
  void useT(a1).read();
  void useT(b1).read();
  return <p>x</p>;
}

// Uncertifiable: the parameter is written before the read.
export function Written() {
  const [a1] = createSignal(0);
  const useT = (n: () => number) => {
    n = () => 2;
    return { read: (): number => n() };
  };
  void useT(a1).read();
  return <p>x</p>;
}

// Clean: the same read in a click handler, where no window is open.
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

// Clean: the parameter holds a setter the memo created, called only in a closure nothing calls. The new row aliases accessors
// only, so ADR 0509's net keeps deciding this write, as on main (the fuzzer's lg-recall `net-param-later`).
export function SetterLater() {
  const m1 = createMemo(() => {
    const later = (g: (v: number) => void) => () => g(1);
    const [, w] = createSignal(0);
    later(w);
    return 1;
  });
  return <p>{m1()}</p>;
}
