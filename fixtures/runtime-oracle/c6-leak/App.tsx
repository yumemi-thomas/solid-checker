import { createSignal, createMemo, onCleanup } from "solid-js";
import * as L from "./lib";

export function Write() {
  const [count, setCount] = createSignal(0);
  const h = L.handlers(() => setCount(count() + 1));
  return <div {...h}>w</div>;
}
export function Owner() {
  const [count] = createSignal(0);
  const h = L.handlers(() => {
    const m = createMemo(() => count());
    onCleanup(() => console.log(m()));
  });
  return <div {...h}>o</div>;
}
export function Props(props: { value: number }) {
  const h = L.handlers(() => console.log(props.value));
  return <div {...h}>p</div>;
}
export function Destr(props: { value: number }) {
  const h = L.handlers(() => {
    const { value } = props;
    console.log(value);
  });
  return <div {...h}>d</div>;
}
