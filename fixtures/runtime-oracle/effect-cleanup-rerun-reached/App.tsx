import { createEffect, createRenderEffect, createSignal, Show } from "solid-js";

// ADR 1183: the cleanup an inline apply returns runs on the effect's rerun, in the apply's strict-read window, when a click
// a user can reach writes a new value to the signal the compute reads: rc.21 warns STRICT_READ_UNTRACKED at its read.

// Violations.
export function EffectRerun() {
  const [r, setR] = createSignal(3);
  const [t, setT] = createSignal(1);
  createEffect(
    () => t(),
    () => () => {
      r();
    },
  );
  void setR;
  return (
    <div>
      <button onClick={() => setT(2)}>x</button>
    </div>
  );
}

export function RenderEffectRerun() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal("a");
  createRenderEffect(
    () => t(),
    () => () => {
      r();
    },
  );
  return <button onClick={() => setT("b")}>x</button>;
}

// Uncertifiable: no rerun is proven.
export function TwinSameValue() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  createEffect(
    () => t(),
    () => () => {
      r();
    },
  );
  return <button onClick={() => setT(1)}>x</button>;
}

export function TwinUnderShow() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  const [on] = createSignal(true);
  createEffect(
    () => t(),
    () => () => {
      r();
    },
  );
  return (
    <Show when={on()}>
      <button onClick={() => setT(2)}>x</button>
    </Show>
  );
}

export function TwinConditionalWrite() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  const [go] = createSignal(false);
  createEffect(
    () => t(),
    () => () => {
      r();
    },
  );
  return (
    <button
      onClick={() => {
        if (go()) setT(2);
      }}
    >
      x
    </button>
  );
}

export function TwinDisabled() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  createEffect(
    () => t(),
    () => () => {
      r();
    },
  );
  return (
    <button disabled onClick={() => setT(2)}>
      x
    </button>
  );
}
