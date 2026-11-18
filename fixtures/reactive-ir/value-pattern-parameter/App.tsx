import { createSignal, createStore } from "solid-js";

const log = (...a: unknown[]) => console.log(...a);

// An accessor handed in a literal to a destructuring parameter: the call `get()` reads it.
function Options() {
  const [a] = createSignal(1);
  const use = ({ get }: { get: () => number }) => get();
  const x = use({ get: a });
  log(x);
  return <p>{a()}</p>;
}

// Twin: a plain function in the literal.
function OptionsPlain() {
  const use = ({ get }: { get: () => number }) => get();
  const x = use({ get: () => 1 });
  log(x);
  return <p>{x}</p>;
}

// Twin: the accessor handed directly.
function OptionsDirect() {
  const [a] = createSignal(1);
  const use = (get: () => number) => get();
  const x = use(a);
  log(x);
  return <p>{a()}</p>;
}

// A props object handed to a helper whose pattern reads `a`.
function pickProp({ a }: { a: number }) {
  return a;
}
function Child(props: { a: number }) {
  const x = pickProp(props);
  log(x);
  return <i>{x}</i>;
}

// Twin: a fresh literal handed to the helper.
function pickPlain({ a }: { a: number }) {
  return a;
}
function ChildPlain(props: { a: number }) {
  const x = pickPlain({ a: 1 });
  log(x, props);
  return <i>{x}</i>;
}

// Twin: the props object read through a plain parameter.
function pickDirect(p: { a: number }) {
  return p.a;
}
function ChildDirect(props: { a: number }) {
  const x = pickDirect(props);
  log(x);
  return <i>{x}</i>;
}

// A store handed to a helper whose pattern reads `a`.
function pickStore({ a }: { a: number }) {
  return a;
}
function StoreHelper() {
  const [st] = createStore({ a: 1 });
  const x = pickStore(st);
  log(x);
  return <p>{x}</p>;
}

// Twin: a plain object handed to the helper.
function pickStorePlain({ a }: { a: number }) {
  return a;
}
function StoreHelperPlain() {
  const st = { a: 1 };
  const x = pickStorePlain(st);
  log(x);
  return <p>{x}</p>;
}

// Twin: the store read through a plain parameter.
function pickStoreDirect(s: { a: number }) {
  return s.a;
}
function StoreHelperDirect() {
  const [st] = createStore({ a: 1 });
  const x = pickStoreDirect(st);
  log(x);
  return <p>{x}</p>;
}

export function App() {
  const [s] = createSignal(1);
  return (
    <div>
      <Options />
      <OptionsPlain />
      <OptionsDirect />
      <Child a={s()} />
      <ChildPlain a={s()} />
      <ChildDirect a={s()} />
      <StoreHelper />
      <StoreHelperPlain />
      <StoreHelperDirect />
    </div>
  );
}
