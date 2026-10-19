import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
declare function opaqueCb(f: () => void): void;
import { X } from "./a";
declare function sink(x: unknown): void;
function helper(v: unknown) {
  sink(v);
}
export function C() {
  {
    const X = { run() {} };
    helper(X);
  }
  void X;
  return <main />;
}
