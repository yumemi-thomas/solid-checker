import { createSignal } from "solid-js";
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
function bare(o: Runner) {
  const { run } = o;
  run();
}
export function C() {
  const [count] = createSignal(0);
  bare({ run() { console.log(count()); } });
  return <main />;
}
