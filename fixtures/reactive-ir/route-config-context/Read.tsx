import { useContext } from "solid-js";
import { RouterContext } from "@solidjs/router";

// The router context's `matches` hold route records with `component`.
export function matches() {
  return useContext(RouterContext);
}
