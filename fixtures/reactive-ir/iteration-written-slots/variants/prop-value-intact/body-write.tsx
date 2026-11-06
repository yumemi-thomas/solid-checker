import { createSignal, createStore } from "solid-js";
function Knob(props: { value: number }) {
  const [draft] = createSignal(props.value);
  return <p>{draft()}</p>;
}
function Dial(props: { value: number }) {
  const [draft] = createSignal(props.value);
  return <p>{draft()}</p>;
}
export function BodyWrite() {
  const [level] = createSignal(1);
  const [state] = createStore({ x: 1 });
  return (
    <div>
      <Knob value={level()} />
      <Dial value={state.x} />
    </div>
  );
}
