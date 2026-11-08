import { createSignal } from "solid-js";

const [, setW] = createSignal(0);
const [items] = createSignal<number[]>([1]);
const [pending] = createSignal(Promise.resolve());

// The program puts a `then` of its own on an object: a value typed as a
// promise may be it.
export const thenable = { then(f: () => void) { f(); } };

export function Unproven() {
  pending().then(() => { setW(1); });
  return <p />;
}

export function Fresh() {
  Promise.resolve().then(() => { setW(2); });
  return <p />;
}

export function Held() {
  const p = Promise.resolve();
  p.then(() => { setW(3); });
  return <p />;
}

export function Escaped() {
  const p = Promise.resolve();
  Object.assign(p, { then: (f: () => void) => { f(); } });
  p.then(() => { setW(4); });
  return <p />;
}

export function OtherMember() {
  items().forEach(() => { setW(5); });
  return <p />;
}

export function Wrapped() {
  (Promise.resolve() as Promise<void>).then(() => { setW(6); });
  return <p />;
}

export function Rebound() {
  let p = Promise.resolve();
  p = pending();
  p.then(() => { setW(7); });
  return <p />;
}
