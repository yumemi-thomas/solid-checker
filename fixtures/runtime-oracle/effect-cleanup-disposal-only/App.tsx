import { createEffect, createRenderEffect, createSignal, createStore, For, Show } from "solid-js";

// ADR 1167: the cleanup an effect's inline apply returns, where the compute provably tracks nothing, runs only when the effect's
// owner is disposed (a `Show` branch or a `For` row removed): no strict-read window, as an `onCleanup` callback's.

function ShowInner() {
  const [s] = createStore({ v: 0 });
  createEffect(
    () => 1,
    () => () => {
      void s.v;
    },
  );
  return <p />;
}

// Clean: the branch's disposal runs the cleanup.
export function ShowRemoved() {
  const [on, setOn] = createSignal(true);
  return (
    <div>
      <Show when={on()}>
        <ShowInner />
      </Show>
      <button onClick={() => setOn(false)}>x</button>
    </div>
  );
}

// Clean: a row's disposal runs the cleanup (owned there; only the read role takes this answer).
export function ForRowRemoved() {
  const [s] = createStore({ v: 0 });
  const [rows, setRows] = createSignal([1]);
  return (
    <div>
      <For each={rows()}>
        {() => {
          createEffect(
            () => 1,
            () => () => {
              void s.v;
            },
          );
          return <p />;
        }}
      </For>
      <button onClick={() => setRows([])}>x</button>
    </div>
  );
}

// Clean: a render effect's returned cleanup, a signal read.
export function RenderEffectCleanup() {
  const [t] = createSignal(1);
  createRenderEffect(
    () => 0,
    () => () => {
      t();
    },
  );
  return <div />;
}

// Uncertifiable: the compute reads `t`, so a rerun may call the cleanup in the apply's window (ADR 0539).
export function TwinComputeTracks() {
  const [s] = createStore({ v: 0 });
  const [t, setT] = createSignal(1);
  createEffect(
    () => t(),
    () => () => {
      void s.v;
    },
  );
  return <button onClick={() => setT(2)}>x</button>;
}

// Uncertifiable: a named apply may be called elsewhere.
export function TwinNamedApply() {
  const [s] = createStore({ v: 0 });
  const apply = () => () => {
    void s.v;
  };
  createEffect(() => 1, apply);
  return <div />;
}

// Uncertifiable: the read is in a function nested in the cleanup.
export function TwinNestedRead() {
  const [s] = createStore({ v: 0 });
  createEffect(
    () => 1,
    () => () => {
      const h = () => s.v;
      return h;
    },
  );
  return <div />;
}

// Uncertifiable: an async cleanup.
export function TwinAsyncCleanup() {
  const [s] = createStore({ v: 0 });
  createEffect(
    () => 1,
    () => async () => {
      void s.v;
    },
  );
  return <div />;
}
