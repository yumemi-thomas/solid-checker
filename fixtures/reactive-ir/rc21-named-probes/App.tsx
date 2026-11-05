import { createMemo, createSignal, createStore, isPending, latest } from "solid-js";

// By name, `latest` and `isPending` read in a strict window as a thunk does.
export function ByName(props: { get: () => number }) {
  const [count] = createSignal(1);
  const memo = createMemo(async () => 1);
  const one = latest(count);
  const two = isPending(count);
  const three = latest(memo);
  const four = isPending(props.get);
  return <p>{[one, two, three, four].length}</p>;
}

// Over a pending store they warn, and throw nothing.
export function PendingStore() {
  const [store] = createStore(async () => ({ a: 1 }), { a: 0 });
  const one = isPending(() => store.a);
  const two = latest(() => store.a);
  return <p>{[one, two].length}</p>;
}

// A handler has no window.
export function Handler() {
  const [count] = createSignal(1);
  return <button onClick={() => latest(count)}>x</button>;
}
