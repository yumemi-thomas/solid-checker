import { createSignal } from "solid-js";
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
export function C() {
  const [count] = createSignal(0);
  Object.values({ run() { console.log(count()); } }).forEach((f) => f());
  return <main />;
}
