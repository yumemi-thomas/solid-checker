import { channelFor } from "./channel.js";

export function PairedView(props) {
  const value = channelFor(props.client);
  return <span>{value}</span>;
}
