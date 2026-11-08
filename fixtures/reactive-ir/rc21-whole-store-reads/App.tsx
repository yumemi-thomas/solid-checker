import { createEffect, createMemo, createStore, deep, snapshot } from "solid-js";

// Each read warns STRICT_READ_UNTRACKED on rc.21 (ADRs 0620, 0621).
export function Reads() {
  const [row] = createStore({ a: 1 });
  const [list] = createStore<number[]>([1]);
  const copy = deep(row);
  const text = JSON.stringify(row);
  const got = Reflect.get(row, "a");
  const values = Object.values(row);
  const entries = Object.entries(row);
  const merged = Object.assign({}, row);
  const spread = { ...row };
  const from = Array.from(list);
  const items = [...list];
  const [first] = list;
  const coerced = `${row}`;
  let sum = 0;
  for (const item of list) sum += item;
  createEffect(
    () => 0,
    () => {
      console.log(deep(row));
    },
  );
  return <p>{[copy, text, got, values, entries, merged, spread, from, items, first, coerced, sum].length}</p>;
}

// An empty array still reads `length`.
export function EmptyArray() {
  const [list] = createStore<number[]>([]);
  const items = [...list];
  return <p>{items.length}</p>;
}

// None of these reads a key: silent on rc.21.
export function Silent() {
  const [row] = createStore({ a: 1 });
  const keys = Object.keys(row);
  const own = Object.hasOwn(row, "a");
  const has = "a" in row;
  const plain = snapshot(row);
  const truthy = !!row;
  for (const key in row) console.log(key);
  return <p>{[keys, own, has, plain, truthy].length}</p>;
}

// Tracked or outside a window: a read, and no defect.
export function Tracked() {
  const [row] = createStore({ a: 1 });
  const copy = createMemo(() => deep(row));
  const click = () => {
    console.log(JSON.stringify(row), { ...row }, Object.values(row));
  };
  return <button onClick={click}>{copy().a}</button>;
}

// An empty store has no key to read yet, and an unknown one may not: the
// read is unproven, never a violation.
export function EmptyObject(props: { init: { a?: number } }) {
  const [empty] = createStore<{ a?: number }>({});
  const [given] = createStore<{ a?: number }>(props.init);
  const one = { ...empty };
  const two = { ...given };
  const three = Object.values(empty);
  const four = Object.entries(given);
  return <p>{[one, two, three, four].length}</p>;
}

// A pending derive throws where a settled store warns: unproven as a warning.
export function PendingDerive() {
  const [row] = createStore(async () => ({ a: 1 }), { a: 0 });
  const copy = deep(row);
  return <p>{copy.a}</p>;
}

// A shadowed name, a plain value and a member are not these reads.
export function Shadowed() {
  const [row] = createStore({ a: { b: 1 } });
  const plain = { a: 1 };
  const local = (value: unknown) => value;
  const JSON = { stringify: local };
  const text = JSON.stringify(row);
  const fine = deep(plain);
  return <p>{[text, fine].length}</p>;
}
