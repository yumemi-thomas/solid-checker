import { action, createEffect, createReaction, createRoot, createSignal, onCleanup } from "solid-js";

// A user effect's `error` member runs from the flush with no owner: an effect
// or a cleanup registered there has none.
export function EffectErrorArm() {
  const [t1, setT1] = createSignal(1);
  createEffect(() => { if (t1() === 2) throw new Error("x"); return 1; }, {
    effect: () => {},
    error: () => {
      createEffect(() => 1, () => {});
      onCleanup(() => {});
      createRoot(() => onCleanup(() => {}));
    },
  });
  return <button onClick={() => setT1(2)}>go</button>;
}

// A reaction's bundle runs its `error` member the same way; a read, a write
// and an action call there are legal, directly or through an IIFE.
export function ReactionErrorArm() {
  const [r1, setR1] = createSignal(0);
  const [t1, setT1] = createSignal(1);
  const act = action(function* () {
    setR1(9);
  });
  const track = createReaction({ effect: () => {}, error: () => {
    r1();
    setR1(1);
    act();
    (() => { r1(); setR1(2); act(); })();
    createEffect(() => 1, () => {});
    onCleanup(() => {});
  } });
  track(() => { if (t1() === 2) throw new Error("x"); });
  return <button onClick={() => setT1(2)}>go</button>;
}

// A bundle with a spread names no member exactly: its owner work is unplaced.
export function SpreadBundle() {
  const [t1, setT1] = createSignal(1);
  const base = { effect: () => {} };
  createEffect(() => { if (t1() === 2) throw new Error("x"); return 1; }, {
    ...base,
    error: () => {
      onCleanup(() => {});
    },
  });
  return <button onClick={() => setT1(2)}>go</button>;
}
