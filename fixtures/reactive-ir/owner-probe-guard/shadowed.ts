import { onCleanup } from "solid-js";

// Violation: a local function spelled `getOwner` is not Solid's, and its
// truthy answer proves no owner.
const getOwner = () => true;

export function registerShadowed(url: string) {
  getOwner() && onCleanup(() => console.log(url));
}

registerShadowed("/shadowed");
