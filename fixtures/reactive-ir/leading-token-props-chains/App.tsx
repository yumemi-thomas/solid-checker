import { createEffect, createSignal, createStore, merge, omit } from "solid-js";

type Inner = { b: string };
type NestedProps = { a: Inner; b: string; get: () => Inner };

// CERTIFIED: `props.a.b` reads the static prop `a` (its head); `b` is a
// member of the object `a` holds, not the reactive prop `b`. `props.get().b`
// reads the static prop `get` and the box it returns.
function Nested(props: NestedProps) {
  const x = props.a.b;
  const y = props.get().b;
  return <div>{x}{y}</div>;
}
// VIOLATION: `props.b` is the reactive prop.
function Direct(props: NestedProps) {
  const x = props.b;
  return <div>{x}</div>;
}

type Pair = { a: string; b: string };
const flag = Math.random() > 0.5;
const plainPair: Pair = { a: "x", b: "y" };
// UNCERTIFIABLE: `(view && props).b` and `(flag ? plainPair : props).b` read
// the reactive prop `b` when the branch yields `props`; a compound receiver
// was dropped (certified) whatever its leading token named.
function BranchReceiver(props: Pair) {
  const view = omit(props, "b");
  const x = (view && props).b;
  const y = (flag ? plainPair : props).b;
  return <div>{x}{y}</div>;
}

type Item = { name: string };
const first: Item = { name: "a" };
const second: Item = { name: "b" };
function ConditionalCaller(props: { v: string }) {
  const v = props.v;
  return <div>{v}</div>;
}
function MemberCallCaller(props: { v: string }) {
  const v = props.v;
  return <div>{v}</div>;
}
function AccessorCaller(props: { v: string }) {
  const v = props.v;
  return <div>{v}</div>;
}

type Opts = { size: number };
const small: Opts = { size: 1 };
const large: Opts = { size: 2 };

type Entry = { label(): string };
function pick(x: { [k: string]: Entry }, k: string) {
  return x[k].label();
}

function Fmt(props: { fmt: () => string }) {
  return <div>{props.fmt() && <div>shown</div>}</div>;
}

export function App() {
  const [id] = createSignal("c1");
  const [count] = createSignal(1);
  const [store] = createStore({ items: [first] as Item[], open: true });
  const inner: Inner = { b: "s" };
  // EFFECT APPLY: the compute returns a string, not the store (before: the
  // apply's `value` was the store, and `value.length` a store-path SC1001);
  // `() => store.items` returns the nested proxy (VIOLATION below).
  createEffect(
    () => (store.open ? "yes" : "no"),
    (value) => {
      console.log(value.length);
    },
  );
  createEffect(
    () => store.items,
    (value) => {
      console.log(value.length);
    },
  );
  // CLEAN: `merge(store.open ? small : large, …)` views a plain object, not
  // the store (before: SC1001 "store spread" and a store-path read).
  const fromView = merge(store.open ? small : large, { x: 1 }).size;
  const spreadView = { ...merge(store.open ? small : large, { y: 1 }) };
  void fromView;
  void spreadView;
  // UNCERTIFIABLE: `pick` reads a store member when `el && entries` yields
  // the store; its leading token `el` (a fresh element) proved the argument
  // non-reactive (before: certified). `pick(entries, "a")` is the control.
  const [entries] = createStore<{ [k: string]: Entry }>({ a: { label: () => "s" } });
  const el = document.createElement("div");
  const picked = pick(el && entries, "a");
  const pickedDirect = pick(entries, "a");
  void picked;
  void pickedDirect;
  return (
    <div>
      <Nested a={inner} b={id()} get={() => inner} />
      <Direct a={inner} b={id()} get={() => inner} />
      <BranchReceiver a="x" b={id()} />
      {/* UNCERTIFIABLE in ConditionalCaller: the conditional's leading token
          was the store, which it only tests (before: a reactive caller, so
          `props.v` was a violation). */}
      <ConditionalCaller v={(store ? first : second).name} />
      {/* UNCERTIFIABLE in MemberCallCaller: `count.toString()` calls no
          accessor (before: reactive, a violation). */}
      <MemberCallCaller v={count.toString()} />
      {/* VIOLATION in AccessorCaller: `count()` is a reactive caller. */}
      <AccessorCaller v={String(count())} />
      {/* CLEAN: `count.toString` is not the accessor (was SC8015). */}
      <Fmt fmt={count.toString} />
    </div>
  );
}

// EFFECT APPLY MEMBERS: the store wraps only a plain object or array member
// (`isWrappable`), so the apply's `value` is a nested proxy only then.
// CLEAN: `record.title` is a string and `record.created` a `Date`, returned
// raw (before: store-path SC1001 at `value.length` and `value.getTime`).
// VIOLATION: `record.meta` is a plain object, a nested proxy.
export function EffectMembers() {
  const [record] = createStore({ title: "abc", created: new Date(), meta: { tags: ["a"] as string[] } });
  createEffect(
    () => record.title,
    (value) => {
      console.log(value.length);
    },
  );
  createEffect(
    () => record.created,
    (value) => {
      console.log(value.getTime());
    },
  );
  createEffect(
    () => record.meta,
    (value) => {
      console.log(value.tags);
    },
  );
  return <div />;
}
