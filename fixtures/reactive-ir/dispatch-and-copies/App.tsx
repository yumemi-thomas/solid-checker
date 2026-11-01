import { action, createMemo, createRoot, createSignal, createTrackedEffect, onCleanup } from "solid-js";

// Violation (ADR 0516): a `const` copy of a callback is that callback.
export function CopyInTimer() {
  const run = () => {
    onCleanup(() => {});
  };
  const copy = run;
  setTimeout(copy, 0);
  return <div />;
}

// Violation: an object literal's member handed whole to a call.
export function SlotInTimer() {
  const box = { run: () => {
    onCleanup(() => {});
  } };
  setTimeout(box.run, 0);
  return <div />;
}

// Negative: the same copy and slot under `createRoot` run owned.
export function CopyInRoot() {
  const run = () => {
    onCleanup(() => {});
  };
  const copy = run;
  createRoot(copy);
  const box = { run: () => {
    onCleanup(() => {});
  } };
  createMemo(box.run);
  return <div />;
}

// Violation: `.call` and `.bind(…)(…)` of a setter in a memo write there.
export function DispatchInMemo() {
  const [v, setV] = createSignal(0);
  createMemo(() => {
    setV.call(null, 1);
    setV.bind(null)(2);
    return 0;
  });
  return <p>{v()}</p>;
}

// Negative: the same dispatches in a handler.
export function DispatchInHandler() {
  const [v, setV] = createSignal(0);
  const save = action(function* () {
    setV(3);
  });
  return (
    <button onClick={() => {
      setV.call(null, 1);
      setV.bind(null)(2);
      save.call(null);
    }}>{v()}</button>
  );
}

// Uncertifiable: `.apply` with an unknown argument list may throw first.
export function ApplyUnknown(props: { args: [number] }) {
  const [v, setV] = createSignal(0);
  createMemo(() => {
    setV.apply(null, props.args);
    return 0;
  });
  return <p>{v()}</p>;
}

// Uncertifiable: `||=` may call the slot's old value; under a handler it is clean.
export function OrAssign() {
  const [v, setV] = createSignal(0);
  let slot: typeof setV | undefined;
  createMemo(() => {
    (slot ||= setV)(1);
    return 0;
  });
  return <button onClick={() => (slot ||= setV)(2)}>{v()}</button>;
}

// Uncertifiable (review): a generator a scheduler calls runs none of its body.
export function GeneratorHanded() {
  const run = function* () {
    onCleanup(() => {});
  };
  setTimeout(run, 0);
  return <div />;
}

// Negative (review): `{}` inherits `toString`, so the default never applies.
export function PrototypeDefault() {
  const [v, setV] = createSignal(0);
  const { toString = setV } = {} as { toString?: typeof setV };
  createMemo(() => {
    toString(1);
    return 0;
  });
  return <p>{v()}</p>;
}

// Violation (slice 1b): a leaf callback handed by name (here a `const` copy)
// runs in the leaf scope, where a pending async read cannot suspend.
export function LeafByCopy() {
  const pending = createMemo(async () => 1);
  const track = () => {
    pending();
  };
  const copy = track;
  createTrackedEffect(copy);
  return <div />;
}

// Not the leaf's: the same callback also runs from a handler.
export function LeafCopyAlsoClicked() {
  const pending = createMemo(async () => 1);
  const track = () => {
    pending();
  };
  const copy = track;
  createTrackedEffect(copy);
  return <button onClick={copy} />;
}
