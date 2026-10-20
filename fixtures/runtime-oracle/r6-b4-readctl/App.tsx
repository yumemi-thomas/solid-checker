import { createSignal } from "solid-js";
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
export function C() {
  const [count] = createSignal(0);
  helper({ run() { console.log(count()); } });
  return <main />;
}
