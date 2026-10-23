import { createMemo, createSignal, createStore } from "solid-js";

// ADR 0369: the producer lists the calls after a dominating await that run
// on the continuation, so none in a getter, an instance field or JSX: those
// run when the getter is read, the class constructed or the JSX rendered
// (under its render effect), on the continuation or not. A read there is
// an obligation, by syntax, not a proven SC1002. Straight-line reads stay
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
