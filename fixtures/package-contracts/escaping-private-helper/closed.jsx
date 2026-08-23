import { ClosedView } from "./closed-view.jsx";

export function App(props) {
  return <ClosedView client={props.client}></ClosedView>;
}

export function Isolated() {
  return null;
}
