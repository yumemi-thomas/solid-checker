import { createMemo, createSignal, createStore, snapshot, untrack } from "solid-js";
import { externalList, externalReady } from "unknown-reactivity";

const staticItems = ["a", "b"];
const staticReady = true;

function ReactiveProps(props: { items: string[]; ready: boolean }) {
  const capturedItems = untrack(() => props.items);
  const capturedReady = untrack(() => props.ready);
  return (
    <section>
      <ul>{props.items.map((item) => item)}</ul>
      {props.ready ? <span>ready</span> : <span>waiting</span>}
      <ul>{capturedItems.map((item) => item)}</ul>
      {capturedReady && <span>captured prop</span>}
    </section>
  );
}

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

export const App = () => {
  const [items] = createSignal(["a", "b"]);
  const [ready] = createSignal(true);
  const [state] = createStore({ items: ["a", "b"], ready: true });
  const memoItems = createMemo(() => items());
  const memoReady = createMemo(() => ready());
  const capturedItems = untrack(items);
  const capturedStoreItems = snapshot(state.items);
  const capturedReady = untrack(ready);
  const capturedStoreReady = snapshot(state).ready;
  const localReady = true;

  return (
    <main>
      <ul>{items().map((item, index) => item + index)}</ul>
      {ready() && <span>ready</span>}
      <ul>{state.items.map((item) => item)}</ul>
      {state.ready && <span>store ready</span>}
      <ul>{memoItems().map((item) => item)}</ul>
      {memoReady() ? <span>memo ready</span> : <span>memo waiting</span>}
      <ReactiveProps items={items()} ready={ready()} />

      <ul>{staticItems.map((item, index) => item + index)}</ul>
      {staticReady && <span>static</span>}
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
