import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
declare function opaqueCb(f: () => void): void;
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
function C(props: { x: number }) {
  console.log(props.x);
  return <main />;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C x={oracleValue()} /></>;
}
