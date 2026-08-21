import { ambiguousTracked } from "./ambiguous";
import { forwardedHelper } from "./external-reexport";
import { helper as bareHelper } from "bare-package";
import { importedTracked as mappedTracked } from "@internal/values";
import * as namespace from "./values";

// The reviewed package contract certifies the external function itself.
export function bareImportShorthand() {
  return { bareHelper };
}

// The same reviewed runtime export remains certifiable behind a relative
// project re-export because TypeFacts preserves its exact runtime identity.
export function externalReexportShorthand() {
  return { forwardedHelper };
}

export function externalReexportStructured() {
  const { active } = forwardedHelper();
  return { active };
}

// Compiler runtime identity closes both tsconfig path mapping and TypeScript's
// extension-priority choice for the ambiguous relative spelling.
export function pathMappedShorthand() {
  return { mappedTracked };
}

export function ambiguousShorthand() {
  return { ambiguousTracked };
}

// A global has no project-local runtime declaration to join, so it remains
// explicitly uncertifiable.
export function globalShorthand() {
  return { structuredClone };
}

// A namespace binding is an exact non-reactive object, not an unresolved
// accessor value — no SC9012.
export function namespaceShorthand() {
  return { namespace };
}
