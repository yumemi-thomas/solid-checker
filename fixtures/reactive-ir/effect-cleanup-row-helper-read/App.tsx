import { For, Show, action, createEffect, createSignal } from "solid-js";

// ADR 1198: a helper the cleanup an inline apply returns calls, inside a `Show` or `For` row, where the compute tracks nothing
// (ADR 1167): it runs at the row's disposal only, outside any strict-read window, so its read is silent, as a read written in the
// cleanup is. The action rule's row at an unselectable callee there stays (ADR 1196).

// Silent: a wrapper helper's read, in a `Show` row.
export function ShowWrapper() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  const wrapUse = () => {
    r();
  };
  return (
    <div>
      <Show when={t()}>
        {() => {
          createEffect(
            () => 1,
            () => () => {
              wrapUse();
            },
          );
          return <p>x</p>;
        }}
      </Show>
      <button onClick={() => setT(0)}>go</button>
    </div>
  );
}

// Silent: a hook's member accessor, in a `For` row.
export function ForMember() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  const useThing = () => ({ useIt: r });
  const thing = useThing();
  return (
    <div>
      <For each={t() ? [1] : []}>
        {() => {
          createEffect(
            () => 1,
            () => () => {
              thing.useIt();
            },
          );
          return <li>x</li>;
        }}
      </For>
      <button onClick={() => setT(0)}>go</button>
    </div>
  );
}

// Uncertifiable: the compute tracks `t`, so a rerun calls the cleanup in the apply's window.
export function TrackedCompute() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  const wrapUse = () => {
    r();
  };
  createEffect(
    () => t(),
    () => () => {
      wrapUse();
    },
  );
  return <button onClick={() => setT(2)}>go</button>;
}

// Uncertifiable, unchanged (ADR 1196): an action through `Object.values` in a `For` row's cleanup.
export function ForActionValues() {
  const [, setW] = createSignal(0);
  const act = action(function* () {
    setW(7);
  });
  const [t, setT] = createSignal(1);
  const box = { useIt: act };
  return (
    <div>
      <For each={t() ? [1] : []}>
        {() => {
          createEffect(
            () => 1,
            () => () => {
              Object.values(box)[0]();
            },
          );
          return <li>x</li>;
        }}
      </For>
      <button onClick={() => setT(0)}>go</button>
    </div>
  );
}
