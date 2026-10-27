import { createEffect, createMemo, createSignal, onCleanup } from "solid-js";

// Violations: the same sites, reached.

export function HandleCalled() {
  const [w, setW] = createSignal(0);
  const h = onCleanup(() => {
    setW(1);
  });
  h();
  return <div>{w()}</div>;
}

// A class declared where its function runs: the static block runs at its
// definition, with no owner in a timer.
export function StaticBlockLive() {
  setTimeout(() => {
    class K {
      static {
        onCleanup(() => {});
      }
    }
  }, 0);
  return <div />;
}

export function IifeChildRendered() {
  async function Child1() {
    await Promise.resolve();
    createEffect(() => 1, () => {});
    return null;
  }
  return <div><Child1 /></div>;
}

export function ClosureOfRenderedChild() {
  const [w1, setW1] = createSignal(0);
  const f1 = () => {
    return setW1(17);
  };
  function Child1() {
    f1();
    return <span />;
  }
  return <div>{w1()}<Child1 /></div>;
}

export function NestedEffectsRendered() {
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
  return <div>{w()}<Child1 /></div>;
}

export function IfTrueInMemo() {
  const [w, setW] = createSignal(0);
  const m = createMemo(() => {
    if (true) setW(1);
    return w();
  });
  return <div>{m()}</div>;
}

export function RuntimeConditionInMemo() {
  const [w, setW] = createSignal(0);
  const [c] = createSignal(true);
  const m = createMemo(() => {
    if (c()) setW(1);
    return w();
  });
  return <div>{m()}</div>;
}

// A function declaration is hoisted: the call before the `return` runs it.
export function HoistedAfterReturnInMemo() {
  const [w, setW] = createSignal(0);
  const m = createMemo(() => {
    bump();
    return w();
    function bump() {
      setW(1);
    }
  });
  return <div>{m()}</div>;
}
