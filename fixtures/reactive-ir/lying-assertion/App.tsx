import { createSignal } from "solid-js";

const [w, setW] = createSignal(0);

export function Lies() {
  const t = { forEach(f: () => void) { queueMicrotask(f); } };
  (t as unknown as number[]).forEach(() => { w(); });
  return <p />;
}

export function Held() {
  const t = { then(f: () => void) { f(); } };
  const q = t as unknown as Promise<void>;
  q.then(() => { setW(1); });
  return <p />;
}

export function OwnType() {
  const p = Promise.resolve();
  (p as Promise<void>).then(() => { setW(1); });
  return <p />;
}

export function Literal() {
  const q = [1] as unknown as number[];
  q.forEach(() => { w(); });
  return <p />;
}

export function Union(value: Promise<void> | { then(f: () => void): void }) {
  (value as Promise<void>).then(() => { setW(1); });
  return <p />;
}
