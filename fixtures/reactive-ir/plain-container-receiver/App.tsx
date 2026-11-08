import { createSignal } from "solid-js";

const [c] = createSignal(1);

class Late<K, V> extends Map<K, V> {
  override forEach(cb: (v: V, k: K, m: Map<K, V>) => void) {
    queueMicrotask(() => {
      super.forEach(cb);
    });
  }
}
const Other = Late;

export function Plain() {
  const m = new Map([[1, 1]]);
  m.forEach(() => {
    c();
  });
  new Map([[1, 1]]).forEach(() => {
    c();
  });
  return <p />;
}

export function Subclass() {
  const m = new Late([[1, 1]]) as Map<number, number>;
  m.forEach(() => {
    c();
  });
  return <p />;
}

export function Alias() {
  const Made = Late;
  const m: Map<number, number> = new Made([[1, 1]]);
  m.forEach(() => {
    c();
  });
  return <p />;
}

export function Shadowed() {
  const Map = Other;
  const m = new Map([[1, 1]]);
  m.forEach(() => {
    c();
  });
  return <p />;
}

export function Rebound() {
  let m = new Map([[1, 1]]);
  m = new Late([[1, 1]]);
  m.forEach(() => {
    c();
  });
  return <p />;
}
