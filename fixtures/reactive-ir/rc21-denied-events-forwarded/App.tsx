import { For, createSignal } from "solid-js";

function Picker(props: { onChange: (v: string) => void; options: string[] }) {
  return <For each={props.options}>{(option) => <input type="radio" onChange={() => props.onChange(option())} />}</For>;
}
function Card(props: { onRange: (v: string) => void }) {
  return <Picker options={["a"]} onChange={(chosen) => props.onRange(chosen)} />;
}
export function C1() {
  const [on, setOn] = createSignal(false);
  const [r, setR] = createSignal("x");
  document.body.focus();
  return <Card onRange={on() ? (v) => setR(v) : (v) => setR(v + r())} />;
}
