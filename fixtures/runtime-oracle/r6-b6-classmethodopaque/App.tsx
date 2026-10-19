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
class K {
  constructor(private read: () => number) {}
  run() { console.log(this.read()); }
}
export function C() {
  const [count] = createSignal(0);
  opaqueFn(new K(count));
  return <main />;
}
