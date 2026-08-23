import { ExprView } from "./expr-view.jsx";

export function App(props) {
  return <ExprView client={props.client}>{props.label}</ExprView>;
}

export function Isolated() {
  return null;
}
