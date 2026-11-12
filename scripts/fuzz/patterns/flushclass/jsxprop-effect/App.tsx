import { createEffect, createSignal, flush } from "solid-js";
const [n] = createSignal(0);
function Child(props: { v: void }) {
  const read = () => props.v;
  createEffect(() => n(), () => { read(); });
  return <p>c</p>;
}
export function App() {
  setTimeout(() => { document.body.append((<Child v={flush()} />) as unknown as Node); });
  return <p>x</p>;
}
