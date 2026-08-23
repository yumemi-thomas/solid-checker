import { Render as InternalRender } from "./internal.js";

export { Panel, Panel as Root } from "./panel.js";

export function UseChannel(props) {
  return InternalRender({ client: props.client });
}

export function Render(input) {
  return { text: String(input) };
}

export function Isolated() {
  return { value: 2 };
}
