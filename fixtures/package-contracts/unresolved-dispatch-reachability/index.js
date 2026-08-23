import { Panel } from "./panel.js";

export function Reaches(props) {
  return Panel({ client: props.client });
}

export function Isolated() {
  return { value: 2 };
}
