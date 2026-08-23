import { HeldView } from "./held-view.jsx";

function Wrap(props) {
  return <span>{props.label}</span>;
}

export function Held(props) {
  return <Wrap label={props.label} child={HeldView} />;
}

export function Isolated() {
  return null;
}
