import { channelFor } from "./channel.js";

export { channelFor } from "./channel.js";

export function forwarded(props) {
  const value = channelFor(props.client);
  return { value };
}

export function Isolated() {
  return { value: 2 };
}
