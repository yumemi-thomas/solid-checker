import { channelFor } from "./channel.js";

export function Panel(props) {
  const value = channelFor(props.client);
  return { value };
}
