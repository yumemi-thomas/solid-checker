import { createSignal } from "solid-js";

// The project loads dep-a only through `import()`: no import declaration names it.
export function App() {
  const [count, setCount] = createSignal(0);
  void import("dep-a").then((mod) => setCount(mod.useBox()));
  return <p>{count()}</p>;
}
