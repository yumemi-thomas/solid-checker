import { createSignal } from "solid-js";

const [, setW] = createSignal(0);

// A `top` of the module's own: its member is no window's.
const top: { setTimeout?: (f: () => void) => number } = {};
top.setTimeout = (f: () => void) => {
  f();
  return 0;
};

export function App() {
  setTimeout(() => { setW(1); }, 0);
  Promise.resolve().then(() => { setW(2); });
  return <p />;
}
