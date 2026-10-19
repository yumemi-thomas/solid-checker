import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
declare function opaqueCb(f: () => void): void;
import { createSignal, createMemo, createEffect, onCleanup, untrack } from "solid-js";
function mix(cb: () => void) {
  const m = createMemo(() => (cb(), 1));
  untrack(cb);
  return m;
}
function C(props: { v: number }) {
  let calls = 0;
  const m = mix(() => { const { v } = props; console.log(v); calls++; });
  return <main id="oracle-mixed">{m()}:{calls}</main>;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C v={oracleValue()} /></>;
}
