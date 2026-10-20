import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
import { createMemo, untrack, createEffect } from "solid-js";
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
function press() {
  helper(opaque);
}
export function C() {
  {
    const press = () => undefined;
    press();
  }
  return <button onClick={press} />;
}
