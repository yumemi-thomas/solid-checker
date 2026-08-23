import { channelFor } from "./channel.js";

export function Show(props) {
  const value = channelFor(props.client);
  return <span>{value}</span>;
}
