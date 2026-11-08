import { createSignal } from "solid-js";

const [, setW] = createSignal(0);

// `top` is the window of an unframed page: this replaces its `setTimeout`.
(top as any).setTimeout = (f: () => void) => {
  f();
  return 0;
};

export function App() {
  setTimeout(() => { setW(1); }, 0);
  Promise.resolve().then(() => { setW(2); });
  return <p />;
}
