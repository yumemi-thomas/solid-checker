import { ambiguousTracked } from "./ambiguous";
import { helper as bareHelper } from "bare-package";
import { importedTracked as mappedTracked } from "@internal/values";
import * as namespace from "./values";

// Each unresolved value is type-correct. The checker cannot prove whether it
// is a reactive return leaf, so every exported shorthand is SC9012 rather than
// a silently omitted structured property.
export function bareImportShorthand() {
  return { bareHelper };
}

export function pathMappedShorthand() {
  return { mappedTracked };
}

export function ambiguousShorthand() {
  return { ambiguousTracked };
}

export function globalShorthand() {
  return { structuredClone };
}

// A namespace binding is an exact non-reactive object, not an unresolved
// accessor value — no SC9012.
export function namespaceShorthand() {
  return { namespace };
}
