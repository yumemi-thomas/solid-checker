import { fireAndFlush } from "./helpers";

const node = document.createElement("input");

// A factory whose returned object holds the helper.
export function renderCard() {
  const open = () => fireAndFlush(() => node.dispatchEvent(new Event("open")));
  return { open };
}
