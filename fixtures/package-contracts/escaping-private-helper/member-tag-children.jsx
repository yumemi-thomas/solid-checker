import * as paired from "./paired-view.jsx";

export function App(props) {
  return <paired.PairedView client={props.client}></paired.PairedView>;
}

export function Isolated() {
  return null;
}
