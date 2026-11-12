import { createMemo, createSignal, resolve } from "solid-js";
function run2(fn: (f: () => number) => Promise<number>, f: () => number) {
  void fn(f);
}
const [n] = createSignal(0);
export function App() {
  const m = createMemo(() => {
    n();
    run2(resolve, () => n());
    return 1;
  });
  return <p>{m()}</p>;
}
