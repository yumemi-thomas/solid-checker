import { onCleanup } from "solid-js";

export let mutable = () => {};
export function replace() {
  mutable = () => { onCleanup(() => {}); };
}
export let reassigned = function () {};
reassigned = () => { onCleanup(() => {}); };
declare function wrap(callback: () => void): () => void;
export const wrapped = wrap(() => {});
