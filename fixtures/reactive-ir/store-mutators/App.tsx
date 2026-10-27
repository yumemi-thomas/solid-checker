import { createStore } from "solid-js";

class Bag {
  push(_value: number) {}
}

const [store, setStore] = createStore({
  items: [3, 1, 2],
  user: { name: "a", nick: undefined as string | undefined },
  bag: new Bag(),
  bytes: new Uint8Array(2),
});
const [other, setOther] = createStore({ n: 0 });

// Positive: each of the nine mutators drops its write.
export function Mutators() {
  store.items.push(1);
  store.items.pop();
  store.items.shift();
  store.items.unshift(0);
  store.items.splice(0, 1);
  store.items.sort();
  store.items.reverse();
  store.items.fill(7);
  store.items.copyWithin(0, 1);
}

// Positive: another store's setter does not enable this proxy.
export function OtherSetter() {
  return <button onClick={() => setOther(() => { store.items.push(1); })}>{other.n}</button>;
}

// Negative: plain data, and copies of store data.
export function PlainArrays() {
  const loose: number[] = [];
  loose.push(1);
  const copy = [...store.items];
  copy.push(9);
  store.items.slice().push(1);
}

// Negative: the method resolves to a project declaration or another built-in.
export function NotArrayPrototype() {
  store.bag.push(1);
  store.bytes.fill(1);
}

// Negative: the store's own setter write-enables its proxy (rc.17 probe), and
// the draft is a different binding.
export function OwnSetter() {
  return (
    <button
      onClick={() => {
        setStore(() => { store.items.push(5); });
        setStore((draft) => { draft.items.push(6); });
      }}
    >
      go
    </button>
  );
}

// Negative: a shadowing parameter is not the store.
export function Shadowed(store: { items: number[] }) {
  store.items.push(1);
}

// Negative: `nick` is not optional (its type includes `undefined`), which is
// TS2790 under `exactOptionalPropertyTypes`, so the delete stays unreported
// (ADR 0460; fixture store-delete).
export function Delete() {
  delete store.user.nick;
}
