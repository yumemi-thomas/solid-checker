import { createMemo, createSignal, createStore } from "solid-js";

// The producer lists every call after a dominating await that is not in an
// arrow, function or method, so also those in a getter, a class body or JSX.
// Those run when the getter is read, the class constructed or the JSX
// rendered (under its render effect), on the continuation or not: a read
// there is uncertifiable, not a proven SC1002. The straight-line reads stay
// violations.

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

export function Control() {
  const [value] = createSignal(1);
  const [store] = createStore({ n: 1 });
  const data = createMemo(async () => {
    await pause();
    return value() + store.n;
  });
  return <div>{data()}</div>;
}

// The getter is read by the component's JSX, not by the memo.
export function ViaGetter() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return { get v() { return value(); } };
  });
  return <div>{data().v}</div>;
}

// The JSX child is read by the render effect the compiler wraps it in.
export function ViaJsx() {
  const [value] = createSignal(1);
  const [store] = createStore({ n: 1 });
  const data = createMemo(async () => {
    await pause();
    return <span>{value() + store.n}</span>;
  });
  return <div>{data()}</div>;
}

// A field initializer runs when the class is constructed.
export function ViaClassField() {
  const [value] = createSignal(1);
  const [store] = createStore({ n: 1 });
  const data = createMemo(async () => {
    await pause();
    const C = class { field = value() + store.n; };
    return new C().field;
  });
  return <div>{data()}</div>;
}
