import { createSignal } from "solid-js";
import { thing } from "dep-api";

declare global {
  interface Api {
    get(): number;
  }
}
declare const api: Api;

// tsc on the project reports TS2349 at `api.get()(count)`: the call
// error is TypeScript's in both programs, and the checker stays silent on it.
export function Panel() {
  const [count] = createSignal(thing);
  api.get()(count);
  return <p>{thing}</p>;
}
