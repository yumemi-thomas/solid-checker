import { Show } from "./show.jsx";

export function App(props) {
  return <Show client={props.client} />;
}

export function Isolated() {
  return null;
}
