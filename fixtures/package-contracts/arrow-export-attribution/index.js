import { channelFor } from "./channel.js";
import { Panel } from "./panel.js";

export const Arrowed = props => Panel({ client: props.client });

export function Declared(props) {
  return Panel({ client: props.client });
}

export const Direct = props => {
  const value = channelFor(props.client);
  return { value };
};

export function Isolated() {
  return { value: 2 };
}
