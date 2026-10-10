import { createSignal, For } from "solid-js";

function Shadowed(props: { value: string }) {
  const For = (input: { each: number[]; children: (row: number) => unknown }) => {
    void input;
    return <p>callback not invoked</p>;
  };
  return <For each={[0]}>{(_row) => {
    const text = props.value;
    return <p>{text}</p>;
  }}</For>;
}
function Real(props: { value: string }) {
  return <For each={[0]}>{(_row) => {
    const text = props.value;
    return <p>{text}</p>;
  }}</For>;
}
export function App() {
  const [live] = createSignal("a");
  return <main><Shadowed value={live()} /><Real value={live()} /></main>;
}
