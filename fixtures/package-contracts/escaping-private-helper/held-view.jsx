import { channelFor } from "./channel.js";

export function HeldView(props) {
  const value = channelFor(props.client);
  return <span>{value}</span>;
}
