import { channelFor } from "./channel.js";

function unreached(client) {
  return { value: channelFor(client) };
}

export function Steady() {
  return { view: 1 };
}
