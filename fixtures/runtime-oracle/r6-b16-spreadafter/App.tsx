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
type Opts = { onClose: () => void; other?: () => void };
function useThing(o: Opts) {
  o.onClose();
}
declare const c: boolean;
declare const rest: Partial<Opts>;
export function C() {
  useThing({ onClose: () => undefined, ...rest });
  return <main />;
}
