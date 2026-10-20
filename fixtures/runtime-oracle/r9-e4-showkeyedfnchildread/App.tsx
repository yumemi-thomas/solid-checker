import { createSignal, Show, For, Match, Switch, Repeat } from "solid-js";
function C(props: { item?: { id: number } }) {
  const [count] = createSignal(0);
  return <Show when={props.item} keyed>{(item) => { console.log(count(), item.id); return <span />; }}</Show>;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C item={{ id: oracleValue() }} /></>;
}
