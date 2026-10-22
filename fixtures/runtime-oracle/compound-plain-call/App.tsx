import type { JSX } from "@solidjs/web";
import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Root(props: { children?: JSX.Element }) {
  return <div>{props.children}</div>;
}

function Panel() {
  const value = count();
  return <b>{value}</b>;
}

const Split = { Root, Panel };

export function App() {
  return <Split.Root>{Split.Panel()}</Split.Root>;
}
