import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
declare function opaqueCb(f: () => void): void;
import { createStore, merge, omit } from "solid-js";
declare function sink(x: unknown): void;
type P = { x: number; y: number };
function now(f: () => void) {
  f();
}
export function C() {
  const [s] = createStore({ x: 1, y: 2 });
  const m = merge({ z: 1 }, s);
  sink(m.x);
  return <main />;
}
