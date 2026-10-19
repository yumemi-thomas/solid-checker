import { createSignal, Show, For, Match, Switch, Repeat } from "solid-js";
function C(props: { items: number[]; x: number }) {
  return <For each={props.items}>{(item) => { console.log(props.x, item); return <span />; }}</For>;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C x={oracleValue()} items={[oracleValue()]} /></>;
}
