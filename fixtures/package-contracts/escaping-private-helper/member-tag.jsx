import * as dotted from "./dotted-view.jsx";

export function App(props) {
  return <dotted.DottedView client={props.client} />;
}

export function Isolated() {
  return null;
}
