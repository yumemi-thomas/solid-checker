import { createMemo, createSignal } from "solid-js";

class Later {
  static get [Symbol.species]() { return Later; }
  forEach(f: () => void) { queueMicrotask(f); }
}
(Array.prototype as any).constructor = Later;

Object.defineProperty(Object.prototype, "then", {
  configurable: true,
  get(this: any) { return Array.isArray(this) && this[0] === 7 ? (r: any) => r({ forEach() {} }) : undefined; },
});

const search: any = { [Symbol.replace]: (s: string, cb: () => void) => { queueMicrotask(cb); return s; } };

export function Species() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => { [1].map((v) => v).forEach(() => { set(1); }); return s(); });
  return <p>{m()}</p>;
}
export function Replace() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => {
    "x".replace(search, (() => { set(1); }) as any);
    "x".replace("x", () => { set(2); return ""; });
    "x".replace(/x/, () => { set(3); return ""; });
    return s();
  });
  return <p>{m()}</p>;
}
export function Thenable() {
  const [s] = createSignal(0);
  const m = createMemo(async () => {
    const xs = await [7];
    xs.forEach(() => { s(); });
    return 1;
  });
  return <p>{String(m())}</p>;
}
