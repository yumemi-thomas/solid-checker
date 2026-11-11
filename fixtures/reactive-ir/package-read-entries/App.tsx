import { createEffect, createSignal } from "solid-js";
import { Counter, Own, Panel, count, quiet } from "dep-read";

export function Dotted() {
  count.call(null);
  return <p>x</p>;
}
export function Tagged() {
  count`x`;
  return <p>x</p>;
}
export function Constructed() {
  new Counter();
  return <p>x</p>;
}
export function Rendered() {
  return <Own />;
}
export function Prop() {
  const [n] = createSignal(0);
  return <Panel value={n()} />;
}
export function Tracked() {
  createEffect(() => quiet(), () => {});
  return <p>x</p>;
}
