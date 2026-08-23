import { channelFor } from "./channel.js";

export function Render(props) {
  const value = channelFor(props.client);
  return { value };
}
