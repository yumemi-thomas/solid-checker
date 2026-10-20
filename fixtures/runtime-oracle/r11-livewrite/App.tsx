import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
declare function opaqueCb(f: () => void): void;
import * as A from "./a";
import { X } from "./a";
declare function sink(x: unknown): void;
function helper(v: unknown) {
  sink(v);
}
export function C() {
  (A as unknown as { X: unknown }).X = { run() {} };
  helper(X);
  return <main />;
}
