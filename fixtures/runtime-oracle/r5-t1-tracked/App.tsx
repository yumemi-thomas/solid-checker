import { createSignal, createMemo } from "solid-js";
import { keep } from "./lib";

function OriginalApp(props: { v: number; api: { go(): void } }) {
  const [count] = createSignal(0);
  const data = createMemo(async () => 1);
  const run = keep(() => {
    console.log(count(), data());
    const { v } = props;
    props.api.go();
    console.log(v);
  });
  return <button onClick={run}>x</button>;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><OriginalApp v={oracleValue()} api={{ go() {} }} /></>;
}
