import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
declare function opaqueCb(f: () => void): void;
import type { JSX } from "@solidjs/web";
import { createSignal, createMemo, createEffect, onCleanup, children } from "solid-js";
function now(f: () => void) {
  f();
}
function C(props: { children?: JSX.Element; x: number }) {
  const c = children(() => (console.log(props.x), props.children));
  const first = c();
  return <main>{first}</main>;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C x={oracleValue()} children={<span>{oracleValue()}</span>} /></>;
}
