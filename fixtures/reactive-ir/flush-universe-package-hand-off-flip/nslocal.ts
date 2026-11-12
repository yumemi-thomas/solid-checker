import { flush } from "solid-js";

// The module a namespace import only calls into (`Local.localStep()`): the namespace object goes to no package.
export function localStep() {
  flush();
}
