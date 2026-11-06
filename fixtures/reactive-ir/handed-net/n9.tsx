import { createSignal, createMemo, createEffect } from "solid-js";
const [sig, setSig] = createSignal(0);
function K(props: { o: { go: () => number } }) {
  props.o.go();
  return <i>x</i>;
}
export function App() {
  return <K o={{ go: sig }} />;
}
