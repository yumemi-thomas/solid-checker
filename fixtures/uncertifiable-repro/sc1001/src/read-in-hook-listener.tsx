// probus-hk geolocation.ts:106-116 and Starred.tsx:359: a component calls a project hook; the hook starts a watcher
// whose listener reads a module signal, and returns the accessor.
import { createSignal } from "solid-js";

const [position, setPosition] = createSignal(0);

function start() {
  window.addEventListener("resize", () => setPosition(position() + 1));
}

function useWatch() {
  start();
  return { position };
}

export function Page() {
  const watch = useWatch();
  return <p>{watch.position()}</p>;
}
