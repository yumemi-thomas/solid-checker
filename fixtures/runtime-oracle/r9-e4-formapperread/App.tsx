import { createSignal, Show, For, Match, Switch, Repeat } from "solid-js";
function C(props: { items: number[] }) {
  const [count] = createSignal(0);
  return <For each={props.items}>{(item) => { console.log(count(), item); return <span />; }}</For>;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C items={[oracleValue()]} /></>;
}
