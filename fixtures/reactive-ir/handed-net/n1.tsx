import { createSignal, createMemo, createEffect } from "solid-js";
const [sig, setSig] = createSignal(0);
function K(props: { o: { go: (v: number) => void } }) {
  createMemo(() => { props.o.go(1); return 0; });
  return <i>x</i>;
}
export function App() {
  return <K o={{ go: setSig }} />;
}
