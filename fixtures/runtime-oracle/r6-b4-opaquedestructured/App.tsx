import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
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
  bare(opaque);
  return <main />;
}
