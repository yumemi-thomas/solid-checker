import "./patches";
import { createSignal } from "solid-js";

const [w, setW] = createSignal(0);

export function Patched() {
  setTimeout(() => { setW(1); }, 0);
  Promise.resolve().then(() => { setW(1); });
  requestAnimationFrame(() => { setW(1); });
  return <p />;
}

export function Unpatched() {
  queueMicrotask(() => { setW(1); });
  [1].map(() => w());
  return <p />;
}
