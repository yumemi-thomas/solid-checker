import { createSignal, Show, For, Match, Switch, Repeat } from "solid-js";
function List(props: { items: number[]; render: (n: number) => unknown }) {
  return <For each={props.items}>{(n) => props.render(n) as any}</For>;
}
function C(props: { items: number[] }) {
  const [count] = createSignal(0);
  return <List items={props.items} render={(n) => { console.log(count(), n); return null; }} />;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C items={[oracleValue()]} /></>;
}
