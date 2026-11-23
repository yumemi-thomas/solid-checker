import { For, action, createEffect, createSignal } from "solid-js";

// ADR 1196: an action called through a value the dispatch cannot select, in the cleanup an inline apply returns inside a `For` row. A click
// empties the list, the row's disposal runs the cleanup under the row's owner, and the action throws ACTION_CALLED_IN_OWNED_SCOPE. The
// compute tracks nothing (ADR 1167), which places a strict read there at the disposal: that answer must not reach the call.

// Uncertifiable at the call (main: only the row at `box`, not at the call Chrome names: a false clean): the call through `Object.values`.
export function ValuesCallee() {
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

// Uncertifiable, unchanged: the call through a member keeps its withholding row at the call.
export function MemberCallee() {
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
              box.useIt();
            },
          );
          return <li>x</li>;
        }}
      </For>
      <button onClick={() => setT(0)}>go</button>
    </div>
  );
}

// Uncertifiable, unchanged: the direct call.
export function DirectCall() {
  const [, setW] = createSignal(0);
  const act = action(function* () {
    setW(7);
  });
  const [t, setT] = createSignal(1);
  return (
    <div>
      <For each={t() ? [1] : []}>
        {() => {
          createEffect(
            () => 1,
            () => () => {
              act();
            },
          );
          return <li>x</li>;
        }}
      </For>
      <button onClick={() => setT(0)}>go</button>
    </div>
  );
}

// Silent, unchanged (ADR 1167): a signal read in the same cleanup runs at the row's disposal, outside any strict-read window.
export function ReadAtDisposal() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  return (
    <div>
      <For each={t() ? [1] : []}>
        {() => {
          createEffect(
            () => 1,
            () => () => {
              r();
            },
          );
          return <li>x</li>;
        }}
      </For>
      <button onClick={() => setT(0)}>go</button>
    </div>
  );
}
