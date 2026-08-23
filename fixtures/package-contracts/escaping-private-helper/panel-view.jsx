import { channelFor } from "./channel.js";

export function PanelView(props) {
  const value = channelFor(props.client);
  return <span>{value}</span>;
}
