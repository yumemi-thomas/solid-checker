import { createEffect, createMemo, createReaction, createSignal } from "solid-js";

function run(cb: () => unknown) {
  cb();
}

// rc.17 runs an effect bundle's `error` member from `runEffect` before it
// opens the strict-read window: reads there are silent, at every layer.
export function ErrorArm() {
  const [count] = createSignal(3);
  const [tick, setTick] = createSignal(1);
  createEffect(
    () => {
      if (tick() === 2) throw new Error("x");
      return 1;
    },
    {
      effect: () => {},
      error: () => {
        count();
        const read = () => {
          count();
        };
        read();
        run(() => count());
        (() => {
          count();
        })();
      },
    },
  );
  return <button onClick={() => setTick(2)}>go</button>;
}

// The bundle's `effect` member is the apply: a read there is a violation,
// written as an arrow or as a method.
export function BundleEffect() {
  const [count] = createSignal(3);
  createEffect(() => 1, {
    effect: () => {
      count();
    },
    error: () => {},
  });
  createEffect(() => 1, {
    effect() {
      count();
    },
    error() {},
  });
  return <button>go</button>;
}

// A bundle whose members are not exact proves no apply: an obligation.
export function SpreadBundle() {
  const [count] = createSignal(3);
  const extra = { error: () => {} };
  createEffect(() => 1, {
    ...extra,
    effect: () => {
      count();
    },
  });
  return <button>go</button>;
}

// A reaction's callback runs as its effect's apply, from the flush, with no
// owner: a write there is legal, also in a layer run in place.
export function ReactionWrite() {
  const [value, setValue] = createSignal(0);
  const [tick, setTick] = createSignal(1);
  const track = createReaction(() => {
    setValue(1);
    run(() => setValue(2));
    (() => {
      setValue(3);
    })();
    const write = () => setValue(4);
    write();
  });
  track(() => tick());
  return <button onClick={() => setTick(2 + value())}>go</button>;
}

// A reaction bundle's `error` member is the error arm, out of the window.
export function ReactionBundle() {
  const [count] = createSignal(3);
  const [tick, setTick] = createSignal(1);
  const track = createReaction({
    effect: () => {},
    error: () => {
      count();
    },
  });
  track(() => tick());
  return <button onClick={() => setTick(2)}>go</button>;
}

// Control: a memo's compute is owned; a write through a layer there throws.
export function MemoWrite() {
  const [value, setValue] = createSignal(0);
  const doubled = createMemo(() => {
    run(() => setValue(5));
    return value() * 2;
  });
  return <button>{doubled()}</button>;
}
