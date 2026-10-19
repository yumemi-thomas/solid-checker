import { createSignal, Show, For, Match, Switch, Repeat } from "solid-js";
function C(props: { n: number }) {
  const [count] = createSignal(0);
  return <Repeat count={props.n}>{(i) => { console.log(count(), i); return <span />; }}</Repeat>;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C n={oracleValue()} /></>;
}
