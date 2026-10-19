import { useThing } from "./lib";
function C(props: { x: number }) {
  useThing({ onClose: () => console.log(props.x), other: () => undefined });
  return <main />;
}

import { createSignal as oracleSignal } from "solid-js";
export function App() {
  const [oracleValue] = oracleSignal(2);
  return <><C x={oracleValue()} /></>;
}
