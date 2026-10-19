import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
declare function opaqueCb(f: () => void): void;
import { createSignal } from "solid-js";
declare function sink(x: unknown): void;
type P = { x: number };
namespace props {
  export const x = 1;
}
function C(_p: P) {
  sink((props as unknown as P).x);
  return <main />;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C x={oracleValue()} /></>;
}
