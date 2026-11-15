import { onCleanup } from "solid-js";

// `lazy` reads the export once the loader's promise settles, and a module's export is live: a `let` rebound later is not the function written here. Open.
export let Late = function Late() {
  onCleanup(() => {});
  return <h4>late</h4>;
};
Late = function Late() {
  return <h4>rebound</h4>;
};
