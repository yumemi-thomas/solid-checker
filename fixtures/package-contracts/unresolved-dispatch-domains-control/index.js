import { channelFor } from "./channel.js";

export function inert(onReady) {
  onReady(2);
  return 2;
}

export function Direct(props, onReady) {
  onReady(1);
  const value = channelFor(props.client);
  return { value };
}
