import { createSignal, Show, For, Match, Switch, Repeat } from "solid-js";
function C(props: { on: boolean }) {
  const [count] = createSignal(0);
  return <Show when={props.on}>{(v) => { console.log(count(), v()); return <span />; }}</Show>;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C on={oracleValue() > 0} /></>;
}
