import { channelFor } from "./channel.js";

export function ExprView(props) {
  const value = channelFor(props.client);
  return <span>{value}</span>;
}
