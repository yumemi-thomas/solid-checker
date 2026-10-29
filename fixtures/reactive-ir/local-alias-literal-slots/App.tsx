import { createMemo, createSignal, createStore } from "solid-js";

// ADR 0480: a slot of a destructured `const` array literal, directly or through
// a `const` holder every reference of which is such a destructuring, and a slot
// of a destructured call of a `const` arrow whose body is an object literal,
// is the `const` it names, for every rule.

// Violations: strict reads in a component body.
export function TupleHolderRead() {
  const [r1] = createSignal(3);
  const pair = [0, r1] as const;
  const [, useIt] = pair;
  useIt();
  return <div></div>;
}

export function ArrayLiteralRead() {
  const [r1] = createSignal(3);
  const [useIt] = [r1];
  useIt();
  return <div></div>;
}

export function ArrowObjectRead() {
  const [r1] = createSignal(3);
  const useThing = () => ({ useIt: r1 });
  const { useIt } = useThing();
  useIt();
  return <div></div>;
}

// Violation: a signal write in a memo through the arrow's slot.
export function ArrowObjectSetterInMemo() {
  const [r1, setR1] = createSignal(0);
  const useThing = () => ({ useIt: setR1 });
  const { useIt } = useThing();
  const m = createMemo(() => {
    useIt(7);
    return 1;
  });
  return <div>{r1()}{m()}</div>;
}

// Violation: a store mutation outside its setter through the holder's slot.
export function TupleHolderStore() {
  const [s1] = createStore({ v: 0 });
  const pair = [s1] as const;
  const [useIt] = pair;
  useIt.v = 7;
  return <div></div>;
}

// Not an alias: the holder is written, so its slot may hold anything.
export function HolderWritten() {
  const [r1] = createSignal(3);
  const pair: Array<() => number> = [r1];
  pair[0] = () => 1;
  const [useIt] = pair;
  useIt();
  return <div></div>;
}

// Not an alias: the holder is handed out.
export function HolderPassed() {
  const [r1] = createSignal(3);
  const pair = [r1];
  console.log(pair);
  const [useIt] = pair;
  useIt();
  return <div></div>;
}

// Not an alias: a block-bodied arrow, a parameter value, a spread before the slot.
export function NotLiteralSlots() {
  const [r1] = createSignal(3);
  const block = () => {
    return { useIt: r1 };
  };
  const { useIt: a } = block();
  a();
  const param = (p: () => number) => ({ useIt: p });
  const { useIt: b } = param(r1);
  b();
  const [, c] = [...[() => 1], r1];
  c();
  return <div></div>;
}

// Silent: the same reads in a memo are tracked.
export function TrackedInMemo() {
  const [r1] = createSignal(3);
  const pair = [r1] as const;
  const [useIt] = pair;
  const m = createMemo(() => useIt());
  return <div>{m()}</div>;
}

// Clean (the fuzzer's legal matrix found these false positives): a write
// through each alias inside its own store's setter lands (ADR 0476).
export function InSetterThroughAliases() {
  const [s1, setS1] = createStore({ v: 0 });
  const pair = [0, s1] as const;
  const [, viaTuple] = pair;
  const useThing = () => ({ viaArrow: s1 });
  const { viaArrow } = useThing();
  return (
    <button
      onClick={() => {
        setS1(() => {
          viaTuple.v = 7;
        });
        setS1(() => {
          viaArrow.v = 8;
        });
      }}
    ></button>
  );
}

// Clean: the setter is reached through a literal slot, and the write is the
// store's own, in that setter's callback.
export function SetterThroughSlot() {
  const [s1, setS1] = createStore({ v: 0 });
  const { set } = { set: setS1 };
  return (
    <button
      onClick={() => {
        set(() => {
          s1.v = 9;
        });
      }}
    ></button>
  );
}
