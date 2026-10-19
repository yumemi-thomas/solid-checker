import { createSignal } from "solid-js";
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
function helperArr(a: Runner[]) {
  a[0].run();
}
export function C() {
  const [count] = createSignal(0);
  helperArr([{ run() { console.log(count()); } }]);
  return <main />;
}
