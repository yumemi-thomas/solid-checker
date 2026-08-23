import { Panel } from "./panel.js";
import { apply } from "./apply.js";

export function Escaped(props) {
  return apply(Panel, props.client);
}

export function Isolated() {
  return { value: 2 };
}
