import { PanelView } from "./panel-view.jsx";

export function App(props) {
  return <PanelView client={props.client} />;
}

export function Isolated() {
  return null;
}
