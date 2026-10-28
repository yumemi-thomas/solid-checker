import { createEffect, createProjection, createStore } from "solid-js";

type State = {
  v?: number;
  a: { b?: number };
  o?: { b?: number };
  nick: string | undefined;
  rec: Record<string, number>;
};

const [store, setStore] = createStore<State>({ v: 0, a: { b: 1 }, nick: undefined, rec: {} });
const [other, setOther] = createStore({ n: 0 });

// Positive: an optional property of the store deleted in the component body,
// and one on a nested store path (strictNullChecks holds the link, ADR 0465).
export function Body() {
  delete store.v;
  delete store.a.b;
}

// Positive: a parenthesized operand, and an optional link narrowed to its
// object (TypeScript accepts both).
export function Narrowed() {
  delete (store.v);
  if (store.o) delete store.o.b;
}

// Positive: in an effect's apply function, and a nested path in an event
// handler.
export function Deferred() {
  createEffect(() => other.n, () => {
    delete store.v;
  });
  return <button onClick={() => { delete store.a.b; }}>go</button>;
}

// Positive: inside a wrapper function.
export function Wrapper() {
  const remove = () => {
    delete store.v;
  };
  remove();
}

// Positive: another store's setter does not enable this proxy.
export function OtherSetter() {
  setOther(() => {
    delete store.v;
  });
}

// Positive: a projection result is read-only outside its compute function.
const selected = createProjection<{ x?: boolean }>(() => {}, {});
export function Projection() {
  delete selected.x;
}

// Negative: the store's own setter write-enables its proxy (rc.17 probe), and
// the draft is a different binding.
export function OwnSetter() {
  setStore(() => {
    delete store.v;
  });
  setStore((draft) => {
    delete draft.v;
    delete draft.a.b;
  });
}

// Negative: a plain object, and a shadowing parameter named like the store.
export function Plain() {
  const local: { v?: number } = { v: 1 };
  delete local.v;
}
export function Shadowed(store: { v?: number }) {
  delete store.v;
}

// Refused (silent): a property whose type includes `undefined` but is not
// optional (TS2790 under `exactOptionalPropertyTypes`), a computed key, an
// optional chain, a cast root, and a `const` alias (the assignment form does
// not follow one either).
export function Refused(key: string) {
  const alias = store;
  delete alias.v;
  delete store.nick;
  delete store.rec[key];
  delete store?.v;
  delete (store as { v?: number }).v;
}
