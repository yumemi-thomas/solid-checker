import { createSignal, Show, For, Match, Switch, Repeat } from "solid-js";
function C(props: { v?: number }) {
  const [count] = createSignal(0);
  return <Switch><Match when={props.v}>{(v) => { console.log(count(), v()); return <span />; }}</Match></Switch>;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C v={oracleValue()} /></>;
}
