import { channelFor } from "./channel.js";

export function div(props) {
  const value = channelFor(props.client);
  return <span>{value}</span>;
}
