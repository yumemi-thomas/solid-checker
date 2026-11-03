import { createEffect, createMemo, createSignal } from "solid-js";

// SC1002 obligation: an arrow slot handed as `box.cb` resolves although a
// nested class writes `this.m`; the assigned member is ADR 0565's hand-off.
export function AfterAwaitAssignThis() {
  const [r1] = createSignal(3);
  const box = { cb: async () => {
    await Promise.resolve();
    class HK {
      m?: () => void;
      constructor() { this.m = () => { r1(); }; }
    }
    new HK().m?.();
    return 1;
  } };
  const m1 = createMemo(box.cb);
  createEffect(() => m1(), () => {});
  return <div />;
}

// SC1002 violation: the resolved arrow reads after its await directly; the
// `this` it holds is the nested class's.
export function AfterAwaitDirect() {
  const [r2] = createSignal(3);
  const box = { cb: async () => {
    class K { owner = this; }
    void K;
    await Promise.resolve();
    r2();
    return 1;
  } };
  const m2 = createMemo(box.cb);
  createEffect(() => m2(), () => {});
  return <div />;
}

// A method's `this` is the object: its slot stays refused, so `box.cb` names
// no exact target.
export function MethodThisRefused() {
  const [r3] = createSignal(3);
  const box = {
    n: 1,
    async cb() {
      await Promise.resolve();
      r3();
      return this.n;
    },
  };
  const m3 = createMemo(box.cb);
  createEffect(() => m3(), () => {});
  return <div />;
}
