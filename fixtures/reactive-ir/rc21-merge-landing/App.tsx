import { createStore, merge } from "solid-js";

// The last source that holds the key answers: a literal's key is no read.
export function LandsPlain() {
  const [store] = createStore({ a: 1 });
  const other = merge(store, { b: 2 });
  const last = merge(store, { a: 2 });
  const blank = merge(store, { a: undefined });
  const one = other.b;
  const two = last.a;
  const three = blank.a;
  const four = merge(store, { b: 2 }).b;
  return <p>{[one, two, three, four].length}</p>;
}

// The store holds the key, and no later source does.
export function LandsOnStore() {
  const [store] = createStore({ a: 1 });
  const view = merge(store, { b: 2 });
  const one = view.a;
  return <p>{one}</p>;
}

// A later source nothing states may hold it, and a view's shape is its sources'.
export function Open(props: { rest: { b: number } }) {
  const [store] = createStore({ a: 1 });
  const unknown = merge(store, props.rest);
  const one = unknown.a;
  const view = merge(store, { b: 2 });
  const copy = { ...view };
  return <p>{[one, copy.a].length}</p>;
}

// One source is that source; through several, a write meets the view.
export function Writes() {
  const [store] = createStore({ a: 1 });
  const same = merge(store);
  const view = merge(store, { b: 2 });
  return (
    <button
      onClick={() => {
        same.a = 2;
        view.a = 3;
      }}
    >
      x
    </button>
  );
}
