import { channelFor } from "./channel.js";

export function ClosedView(props) {
  const value = channelFor(props.client);
  return <span>{value}</span>;
}
