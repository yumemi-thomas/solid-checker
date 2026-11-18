// The barrel re-exports dynamic-import-holder.ts, and its loader hands the module it loads on (kui's
// shell/src/features/registry.ts `import("@kui/feature-…").then(featureModule)`; solid-relay's tests read
// the package index so): every export may be called by code that receives it, so the load hands out the
// whole barrel. (ADR 1071: a load read only as `m.name` or a destructured name hands out that name.)
export { createLoader, createPlain } from "./dynamic-import-holder";

declare function featureModule(module: unknown): void;

export function loadAll() {
  return import("./dynamic-import-barrel").then(featureModule);
}
