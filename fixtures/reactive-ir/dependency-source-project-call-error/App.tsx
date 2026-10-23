import { createSignal } from "solid-js";
import { thing } from "dep-api";

declare global {
  interface Api {
    get(): number;
  }
}
declare const api: Api;

// tsc on the project reports TS2349 at the outer call itself: the call
// error is TypeScript's in both programs, and the checker stays silent on it.
export function Panel() {
  const [count] = createSignal(thing);
  const run = api.get();
  run(count);
  return <p>{thing}</p>;
}
