import { channelFor } from "./channel.js";

export function DottedView(props) {
  const value = channelFor(props.client);
  return <span>{value}</span>;
}
