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
function C(props: { children?: JSX.Element }) {
  const data = createMemo(async () => 1);
  const c = children(() => (console.log(data()), props.children));
  return <main>{c()}</main>;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C children={<span>{oracleValue()}</span>} /></>;
}
