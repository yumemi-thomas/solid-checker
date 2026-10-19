import { createSignal } from "solid-js";
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
function bare(f: () => void) {
  const g = f;
  g();
}
export function C() {
  const [count] = createSignal(0);
  bare(() => console.log(count()));
  return <main />;
}
