import { For } from "solid-js";

export function Rows(props: { value: { text: string } }) {
  return <For each={[0]}>{(_row) => {
    const text = props.value.text; // Violation through exact cross-file forward.
    return <p>{text}</p>;
  }}</For>;
}
