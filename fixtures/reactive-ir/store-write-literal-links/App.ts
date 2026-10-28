import { createStore } from "solid-js";

declare function consume(value: unknown): void;
declare function makeLink(): { v: number };

// ADR 0491: a link past the root that holds the literal the store's own
// initializer wrote, which nothing can replace or reach raw, is wrapped, so
// the write below it is dropped.

// Violations: links that are inline object and array literals.
export function Literal() {
  const [store] = createStore({ a: { v: 0 }, items: [0], deep: { b: { c: 0 } } });
  store.a.v = 1;
  store.items.push(1);
  store.deep.b.c = 1;
  consume(store.a.v);
}

// Violation: through an exact `const` alias, and an unused setter slot.
export function Alias() {
  const [store, setStore] = createStore({ a: { v: 0 } });
  const view = store;
  view.a.v = 1;
}

// Uncertifiable: the setter is referenced, so a link may be replaced.
export function SetterUsed() {
  const [store, setStore] = createStore({ a: { v: 0 } });
  store.a.v = 1;
  setStore((draft) => {
    draft.a = makeLink();
  });
}

// Uncertifiable: the link's value is not a literal.
export function FrozenLink() {
  const [store] = createStore({ a: makeLink() });
  store.a.v = 1;
}

// Uncertifiable: a proxy below the root leaves as a value.
export function Escapes() {
  const [store] = createStore({ a: { v: 0 } });
  consume(store.a);
  store.a.v = 1;
}

// Uncertifiable: the store itself leaves as a value.
export function RootEscapes() {
  const [store] = createStore({ a: { v: 0 } });
  consume(store);
  store.a.v = 1;
}

// Uncertifiable: the initializer is not an inline literal.
export function NamedInitial() {
  const initial = { a: { v: 0 } };
  const [store] = createStore(initial);
  store.a.v = 1;
}

// Uncertifiable: a spread leaves the literal open.
export function Spread(extra: { b: number }) {
  const [store] = createStore({ ...extra, a: { v: 0 } });
  store.a.v = 1;
}

// Uncertifiable: a getter computes the link.
export function Getter() {
  const [store] = createStore({
    get a() {
      return { v: 0 };
    },
  });
  store.a.v = 1;
}

// Uncertifiable: a comparator receives the elements.
export function SortCallback() {
  const [store] = createStore({ items: [{ v: 0 }], a: { v: 0 } });
  store.items.sort(() => 0);
  store.a.v = 1;
}

// Uncertifiable: a module-scope store may be reached from elsewhere.
const [shared] = createStore({ a: { v: 0 } });
export function ModuleScope() {
  shared.a.v = 1;
}
