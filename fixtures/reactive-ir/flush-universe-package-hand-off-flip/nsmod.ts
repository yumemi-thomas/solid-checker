import { flush } from "solid-js";

// The module a namespace import hands whole (`runNs(NS)`): each export is a member of the namespace object the package runs.
export function nsStep() {
  flush();
}
