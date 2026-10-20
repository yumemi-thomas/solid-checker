import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
declare function opaqueCb(f: () => void): void;
import { createSignal, createMemo, createStore, merge, omit, onSettled, untrack } from "solid-js";
function now(f: () => void) {
  f();
}
function later(f: () => void) {
  setTimeout(f, 0);
}
function Btn(p: { onPress: () => void }) {
  return <button onClick={() => p.onPress()} />;
}
function C(props: { x: number }) {
  return <Btn onPress={() => console.log(props.x)} />;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C x={oracleValue()} /></>;
}
