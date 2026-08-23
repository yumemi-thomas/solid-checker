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

export function Arrow(props, onReady) {
  onReady(1);
  const read = () => channelFor(props.client);
  return { read };
}

export function Helper(props, onReady) {
  onReady(1);
  function compute() {
    return channelFor(props.client);
  }
  return { compute };
}
