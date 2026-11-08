import { createEffect, createMemo, createStore, snapshot, untrack } from "solid-js";

// `untrack` scopes the wrapped read: the proxy it returns warns on its next Get.
export function Escaped() {
  const [store] = createStore({ o: { x: 1 }, list: [1, 2], n: "a" });
  const o = untrack(() => store.o);
  const one = o.x;
  const two = untrack(() => store.o).x;
  const { x } = untrack(() => store.o);
  const list = untrack(() => store.list);
  const whole = untrack(() => store);
  const three = whole.n;
  const four = list.length;
  return <p>{[one, two, x, three, four].length}</p>;
}

// In an effect's apply the window is the apply's.
export function InApply() {
  const [store] = createStore({ o: { x: 1 } });
  createEffect(
    () => 1,
    () => {
      const o = untrack(() => store.o);
      console.log(o.x);
    },
  );
  return <p>{1}</p>;
}

// Tracked or event code reads it freely.
export function Tracked() {
  const [store] = createStore({ o: { x: 1 } });
  const o = untrack(() => store.o);
  const memo = createMemo(() => o.x);
  return <button onClick={() => console.log(o.x)}>{memo()}{o.x}</button>;
}

// No proxy: a primitive, a snapshot, a plain value.
export function Plain() {
  const [store] = createStore({ n: "a", o: { x: 1 } });
  const n = untrack(() => store.n);
  const copy = untrack(() => snapshot(store.o));
  const one = n.length;
  const two = copy.x;
  return <p>{[one, two].length}</p>;
}

function pick(store: { o: { x: number } }) {
  return untrack(() => store.o);
}
function read(value: { x: number }) {
  return value.x;
}

// Nothing proves what these hold, or where they are read: obligations.
export function Unproved(props: { o: { x: number } }) {
  const [store] = createStore({ o: { x: 1 }, d: new Date(0) });
  const fromProp = untrack(() => props.o);
  const date = untrack(() => store.d);
  const fromHelper = pick(store);
  const handed = untrack(() => store.o);
  const spread = { ...untrack(() => store.o) };
  const one = fromProp.x;
  const two = date.getTime();
  const three = fromHelper.x;
  const four = read(handed);
  return <p>{[one, two, three, four, spread.x].length}</p>;
}
