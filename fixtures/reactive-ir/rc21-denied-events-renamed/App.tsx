import { createSignal } from "solid-js";

function Field(props: { save: () => void }) {
  return <button onBlur={props.save}>go</button>;
}

export function App() {
  const [count, setCount] = createSignal(0);
  return <Field save={() => { console.log(count()); setCount(1); }} />;
}
