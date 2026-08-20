import { createMemo, createSignal, untrack } from "solid-js";
import { createStore } from "solid-js/store";
import { externalList, externalReady } from "unknown-reactivity";

const staticItems = ["a", "b"];

function ShadowedSources() {
  const createSignal = <T,>(value: T) => [() => value] as const;
  const [items] = createSignal(["shadowed"]);
  const [ready] = createSignal(true);
  return (
    <section>
      <ul>{items().map((item) => item)}</ul>
      {ready() && <span>shadowed</span>}
    </section>
  );
}

export const App = (props: { ready: boolean }) => {
  const [items] = createSignal(["a", "b"]);
  const [ready] = createSignal(true);
  const [state] = createStore({ items: ["a", "b"], ready: true });
  const memoItems = createMemo(() => items());
  const memoReady = createMemo(() => ready());
  const capturedItems = untrack(items);
  const capturedStoreItems = untrack(() => state.items.slice());
  const capturedReady = untrack(ready);
  const capturedStoreReady = untrack(() => state.ready);
  const localReady = true;

  return (
    <main>
      <ul>{items().map((item) => item)}</ul>
      {ready() && <span>ready</span>}
      <ul>{state.items.map((item) => item)}</ul>
      {state.ready ? <span>ready</span> : <span>waiting</span>}
      <ul>{memoItems().map((item) => item)}</ul>
      {memoReady() ? <span>memo ready</span> : <span>memo waiting</span>}

      <ul>{staticItems.map((item) => item)}</ul>
      {props.ready && <span>unproven prop</span>}
      <ul>{capturedItems.map((item) => item)}</ul>
      <ul>{capturedStoreItems.map((item) => item)}</ul>
      {capturedReady && <span>captured accessor</span>}
      {capturedStoreReady ? <span>captured store</span> : <span>waiting</span>}
      <ul>{externalList().map((item) => item)}</ul>
      {externalReady() && <span>external</span>}
      <ul>{staticItems.map((item) => <span title={String(ready())}>{item}</span>)}</ul>
      {localReady && <span title={String(ready())}>branch read</span>}
      {false ? <span title={String(ready())}>consequent read</span> : <span>literal</span>}
      <ShadowedSources />
    </main>
  );
};
