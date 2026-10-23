import { createSignal } from "solid-js";
import { thing } from "dep-api";

declare global {
  interface API {
    get(): (source: () => number) => void;
  }
}
declare const api: API;

// tsc on the project: no error (the package's source is not in its program).
// The dependency-source program lists the package source after App.tsx, so
// its `get(): number` overload comes first and the outer call is TS2349 there
// only: an uncertifiable call, never a silent TypeScript-owned one.
export function Panel() {
  const [count] = createSignal(thing);
  const run = api.get();
  run(count);
  return <p>{thing}</p>;
}
