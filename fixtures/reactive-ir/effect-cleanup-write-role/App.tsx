import { action, createEffect, createMemo, createSignal, For, refresh } from "solid-js";

const go = action(function* () {});

// ADR 1037: the cleanup an inline effect apply returns runs on a rerun (the queue's drain, no owner) or at a disposal (the write guard
// stands down, a `For` row's disposal included): a setter or `refresh` written directly in it is legal on every run. Silent.
export function CleanupWrite() {
  const [w, setW] = createSignal(0);
  const [t, setT] = createSignal(1);
  createEffect(
    () => t(),
    () => () => {
      setW(w() + 1);
    },
  );
  return <button onClick={() => setT(2)}>go</button>;
}

export function CleanupRefresh() {
  const m = createMemo(() => 1);
  const [t, setT] = createSignal(1);
  createEffect(
    () => t(),
    () => () => {
      refresh(m);
    },
  );
  return <button onClick={() => setT(2)}>go</button>;
}

// A `For` row's disposal runs the cleanup under the row's owner; the write is still exempt.
export function CleanupWriteInRow() {
  const [w, setW] = createSignal(0);
  const [items, setItems] = createSignal([1]);
  return (
    <div>
      <For each={items()}>
        {() => {
          createEffect(
            () => 0,
            () => () => {
              setW(1);
            },
          );
          return <p>row</p>;
        }}
      </For>
      <button onClick={() => setItems([])}>remove</button>
    </div>
  );
}

// Unchanged (obligations): an action in the cleanup throws under a `For` row's disposal, so the shared write role is not changed.
export function CleanupAction() {
  const [t, setT] = createSignal(1);
  createEffect(
    () => t(),
    () => () => {
      go();
    },
  );
  return <button onClick={() => setT(2)}>go</button>;
}

// Clean since ADR 1200: a named apply whose one reference is the apply argument is the inline apply.
export function NamedApply() {
  const [w, setW] = createSignal(0);
  const [t, setT] = createSignal(1);
  const apply = () => () => {
    setW(1);
  };
  createEffect(() => t(), apply);
  return <button onClick={() => setT(2)}>go</button>;
}

// A write in a closure called once inside the cleanup: run in place there, so clean since ADR 1195.
export function NestedInCleanup() {
  const [w, setW] = createSignal(0);
  const [t, setT] = createSignal(1);
  createEffect(
    () => t(),
    () => () => {
      const later = () => setW(1);
      later();
    },
  );
  return <button onClick={() => setT(2)}>go</button>;
}
