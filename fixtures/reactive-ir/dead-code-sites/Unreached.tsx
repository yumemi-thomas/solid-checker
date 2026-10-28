import { createEffect, createMemo, createSignal, onCleanup } from "solid-js";

// Uncertifiable: no execution reaches the site (ADR 0458).

// The right of `false &&` never runs, also when a handle call runs the
// callback (fuzz m1).
export function DeadAndOperand() {
  const [w, setW] = createSignal(0);
  const h = onCleanup(() => {
    false && setW(1);
  });
  h();
  return <div>{w()}</div>;
}

// The IIFE's result, the only place `Child1` goes, is discarded (fuzz m2).
export function IifeResultDiscarded() {
  (() => {
    async function Child1() {
      await Promise.resolve();
      createEffect(() => 1, () => {});
      return null;
    }
    return Child1;
  })();
  return <div />;
}

// A class declared after a `return` is never defined, so its static block
// never runs (fuzz m3).
export function ClassAfterReturn() {
  setTimeout(() => {
    return;
    class K {
      static {
        onCleanup(() => {});
      }
    }
  }, 0);
  return <div />;
}

export function CleanupAfterReturn() {
  setTimeout(() => {
    return;
    onCleanup(() => {});
  }, 0);
  return <div />;
}

// `f1` is called only from `Child1`, which nothing renders or names (fuzz m4).
export function ClosureOfUnreferencedChild() {
  const [w1, setW1] = createSignal(0);
  const f1 = () => {
    return setW1(17);
  };
  function Child1() {
    f1();
    return <span />;
  }
  return <div>{w1()}</div>;
}

// An async child nothing renders, with nested effects (fuzz m5).
export function NestedEffectsUnrendered() {
  const [w, setW] = createSignal(0);
  async function Child1() {
    await Promise.resolve();
    createEffect(() => 1, () => {
      createEffect(() => 1, () => {
        setW(43);
      });
    });
    return <span />;
  }
  return <div>{w()}</div>;
}

export function AsyncUnreferenced() {
  async function Child1() {
    await Promise.resolve();
    createEffect(() => 1, () => {});
    return null;
  }
  return <div />;
}

export function IfFalseInMemo() {
  const [w, setW] = createSignal(0);
  const m = createMemo(() => {
    if (false) setW(1);
    return w();
  });
  return <div>{m()}</div>;
}

export function TernaryFalseArmInMemo() {
  const [w, setW] = createSignal(0);
  const m = createMemo(() => (false ? setW(1) : w()));
  return <div>{m()}</div>;
}

export function OrTrueInMemo() {
  const [w, setW] = createSignal(0);
  const m = createMemo(() => true || setW(1));
  return <div>{w()}{String(m())}</div>;
}

export function AfterThrowInMemo() {
  const [w, setW] = createSignal(0);
  const [c] = createSignal(false);
  const m = createMemo(() => {
    if (c()) {
      throw new Error("stop");
      setW(1);
    }
    return w();
  });
  return <div>{m()}</div>;
}

// A method of a class nothing names: no instance exists to call it on.
export function UnreferencedClassMethod() {
  const [w, setW] = createSignal(0);
  const m = createMemo(() => {
    class Counter {
      bump() {
        setW(1);
      }
    }
    return w();
  });
  return <div>{m()}</div>;
}
