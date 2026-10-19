import { createSignal } from "solid-js";
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
function fwd(o: Runner) {
  return () => helper(o);
}
export function C() {
  const [count] = createSignal(0);
  fwd({ run() { console.log(count()); } })();
  return <main />;
}
