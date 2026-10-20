import { createSignal, createMemo, createStore } from "solid-js";
function now(f: () => void) {
  f();
}
function useV(o: { readonly count: number }) {
  return o.count;
}
function C(props: { items: number[] }) {
  const n = useV({ get count() { return props.items.length; } });
  return <main>{n}</main>;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C items={[oracleValue()]} /></>;
}
