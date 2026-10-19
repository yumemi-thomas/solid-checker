import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
declare function opaqueCb(f: () => void): void;
import { createStore, merge, omit } from "solid-js";
declare function sink(x: unknown): void;
type P = { x: number; y: number; ref?: unknown };
function C(props: P) {
  const o = omit(omit(props, "y"), "ref");
  sink(o.x);
  return <main />;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C x={oracleValue()} y={oracleValue()} /></>;
}
