import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
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
  fwd({ run() { console.log(count()); } });
  return <main />;
}
