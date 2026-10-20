import { createSignal, createMemo, createStore, merge, omit, onSettled, untrack } from "solid-js";
function now(f: () => void) {
  f();
}
function later(f: () => void) {
  setTimeout(f, 0);
}
function C(props: { x: number; y: number }) {
  const o = omit(props, "y");
  now(() => console.log(o.x, props.y));
  return <main />;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C x={oracleValue()} y={oracleValue()} /></>;
}
