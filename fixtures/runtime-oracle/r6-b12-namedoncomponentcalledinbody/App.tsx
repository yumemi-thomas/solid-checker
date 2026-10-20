import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
import { createMemo, untrack, createEffect } from "solid-js";
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
function press() {
  helper(opaque);
}
function Btn(props: { onClick: () => void }) {
  props.onClick();
  return <button />;
}
export function C() {
  return <Btn onClick={press} />;
}
