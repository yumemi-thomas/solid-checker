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

// Uncertifiable: a regular expression is served raw, so the write lands.
export function RegexLink() {
  const [store] = createStore({ r: /x/g });
  store.r.lastIndex = 5;
}

// Uncertifiable: a primitive cast to an object is served raw (the write throws).
export function CastPrimitive() {
  const [store] = createStore({ a: 0 as unknown as { v: number } });
  store.a.v = 1;
}

// Uncertifiable: `reverse` may write nothing.
export function Reverse() {
  const [store] = createStore({ items: [0] });
  store.items.reverse();
}

// Violation (ADR 0497): the store reaches the write through an object-literal
// arrow's slot and a tuple holder's slot, consumed only by destructuring.
export function HookSlot() {
  const [store] = createStore({ a: { v: 0 } });
  const useThing = () => ({ useIt: store });
  const { useIt } = useThing();
  useIt.a.v = 1;
}

export function TupleSlot() {
  const [store] = createStore({ items: [0] });
  const pair = [0, store] as const;
  const [, held] = pair;
  held.items.push(1);
}

// Uncertifiable: a rest element receives the holder's tail, store included.
export function TupleRest() {
  const [store] = createStore({ a: { v: 0 } });
  const pair = [0, store] as const;
  const [, held] = pair;
  const [...all] = pair;
  consume(all);
  held.a.v = 1;
}

// Uncertifiable: one call of the hook keeps the whole object.
export function HookKept() {
  const [store] = createStore({ a: { v: 0 } });
  const useThing = () => ({ useIt: store });
  const { useIt } = useThing();
  consume(useThing());
  useIt.a.v = 1;
}

// Uncertifiable: a nested pattern reads a proxy below the store out.
export function HookNested() {
  const [store] = createStore({ a: { v: 0 } });
  const useThing = () => ({ useIt: store });
  const { useIt } = useThing();
  const {
    useIt: { a },
  } = useThing();
  consume(a);
  useIt.a.v = 1;
}

// Uncertifiable (ADR 0497 review): a spread ahead of the store shifts its
// index, so a slot that may receive it is no alias.
export function TupleSpread(xs: number[]) {
  const [store] = createStore({ a: { v: 0 } });
  const pair = [...xs, store] as const;
  const [first] = pair;
  consume(first);
  store.a.v = 1;
}

// Uncertifiable: a quoted key next to a method; the slot receiving the store
// is no recorded alias.
export function HookQuotedKey() {
  const [store] = createStore({ a: { v: 0 } });
  const useThing = () => ({ "useIt": store, m() {} });
  const { useIt } = useThing();
  consume(useIt);
  store.a.v = 1;
}

// Uncertifiable: an escaped pattern key next to a method.
export function HookEscapedKey() {
  const [store] = createStore({ a: { v: 0 } });
  const useThing = () => ({ useIt: store, m() {} });
  const { useIt: held } = useThing();
  consume(held);
  store.a.v = 1;
}

// Violation (ADR 0500): through a hook holder read only by static members.
export function HookMember() {
  const [store] = createStore({ a: { v: 0 } });
  const useThing = () => ({ useIt: store });
  const thing = useThing();
  thing.useIt.a.v = 1;
}

// Silent, a known miss (no ADR 0481 alias, so the root is not proven the
// store): the holder object leaves as a value.
export function HookMemberKept() {
  const [store] = createStore({ a: { v: 0 } });
  const useThing = () => ({ useIt: store });
  const thing = useThing();
  consume(thing);
  thing.useIt.a.v = 1;
}

// Silent, a known miss (no ADR 0481 alias, so the root is not proven the
// store): a member of the holder is written.
export function HookMemberWritten() {
  const [store] = createStore({ a: { v: 0 } });
  const useThing = () => ({ useIt: store, n: 0 });
  const thing = useThing();
  thing.n = 1;
  thing.useIt.a.v = 1;
}

// Silent, a known miss (no ADR 0481 alias, so the root is not proven the
// store): a computed member reads the holder.
export function HookMemberComputed() {
  const [store] = createStore({ a: { v: 0 } });
  const useThing = () => ({ useIt: store });
  const thing = useThing();
  consume(thing["useIt"]);
  thing.useIt.a.v = 1;
}

// Uncertifiable: the store leaves through the member alias.
export function HookMemberPassed() {
  const [store] = createStore({ a: { v: 0 } });
  const useThing = () => ({ useIt: store });
  const thing = useThing();
  consume(thing.useIt);
  thing.useIt.a.v = 1;
}

// Silent (ADR 0500 review): `valueOf` is no key of the literal, so it reads
// `Object.prototype` and hands the holder out; no member alias is recorded.
export function HookMemberValueOf() {
  const [store] = createStore({ a: { v: 0 } });
  const useThing = () => ({ useIt: store });
  const thing = useThing();
  Object.assign(thing.valueOf(), { useIt: { a: { v: 0 } } });
  thing.useIt.a.v = 1;
}

// Silent: a `function` value runs with the holder as `this` and may replace
// the slot; no member alias is recorded.
export function HookMemberThisReset() {
  const [store] = createStore({ a: { v: 0 } });
  const useThing = () => ({
    useIt: store,
    reset: function (this: { useIt: { a: { v: number } } }) {
      this.useIt = { a: { v: 0 } };
    },
  });
  const thing = useThing();
  thing.reset();
  thing.useIt.a.v = 1;
}
