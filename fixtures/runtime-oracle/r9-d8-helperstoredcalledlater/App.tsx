import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
declare function opaqueCb(f: () => void): void;
import { createSignal } from "solid-js";
function now(f: () => void) {
  f();
}
declare function sink(x: unknown): void;
function C(props: { x: number }) {
  let saved: (() => number) | undefined;
  now(() => { const g = () => props.x; saved = g; });
  sink(saved?.());
  return <main />;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C x={oracleValue()} /></>;
}
