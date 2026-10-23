import { createSignal } from "solid-js";
import { thing } from "dep-api";

declare global {
  interface API {
    get(): number;
  }
}
declare const api: API;

export function Panel() {
  const [count] = createSignal(thing);
  const run = api.get();
  run(count);
  return <p>{thing}</p>;
}
