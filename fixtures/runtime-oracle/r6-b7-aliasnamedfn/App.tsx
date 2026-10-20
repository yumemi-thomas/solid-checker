import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
import { createSignal } from "solid-js";
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
function fwd(o: Runner) {
  const p = o;
  helper(p);
}
export function C() {
  const [count] = createSignal(0);
  function run() {
    console.log(count());
  }
  fwd({ run });
  return <main />;
}
