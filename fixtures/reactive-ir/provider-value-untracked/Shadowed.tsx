import { createSignal } from "solid-js";

const [count] = createSignal(1);

// Negative: not a context; a component named like one is a component, and its
// `value` prop is a getter the component reads however it likes.
function Theme(props: { value: number; children?: unknown }) {
  return <div>{props.value}</div>;
}

export function Component() {
  return <Theme value={count()} />;
}
