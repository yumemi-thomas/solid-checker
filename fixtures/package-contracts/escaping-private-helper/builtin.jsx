import { For } from "solid-js";
import { ListedView } from "./listed-view.jsx";

export function App(props) {
  return (
    <For each={props.rows}>{() => <ListedView client={props.client} />}</For>
  );
}

export function Isolated() {
  return null;
}
