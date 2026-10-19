import { createSignal } from "solid-js";
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
function fwd(o: Runner) {
  helper({ ...o });
}
export function C() {
  const [count] = createSignal(0);
  function run() {
    console.log(count());
  }
  fwd({ run });
  return <main />;
}
