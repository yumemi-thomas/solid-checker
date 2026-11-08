import { createMemo, createSignal } from "solid-js";

const own = EventTarget.prototype.addEventListener;
const sync = function (this: any, t: string, cb: any, ...r: any[]) {
  if (t === "x-sync") cb();
  else own.call(this, t, cb, ...r);
};
Object.setPrototypeOf(document, Object.create(Object.getPrototypeOf(document), { addEventListener: { value: sync } }));
export const listener = { addEventListener: sync };

export function App() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => {
    document.addEventListener("x-sync", () => { set(1); });
    return s();
  });
  return <p>{m()}</p>;
}
