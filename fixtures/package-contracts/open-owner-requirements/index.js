import { onCleanup } from "solid-js";
import { undescribedValue } from "partial-contract-package";

// Registers a cleanup on every call, then calls the export the dependency's
// contract does not describe.
export function cleanupThenUndescribed(callback) {
  onCleanup(() => {});
  return undescribedValue(callback);
}

// The same, with the cleanup registered on some calls only.
export function conditionalCleanupThenUndescribed(flag, callback) {
  if (flag) onCleanup(() => {});
  return undescribedValue(callback);
}

// One function under two names.
function registerCleanup(fn) {
  onCleanup(fn);
}

export { registerCleanup, registerCleanup as addCleanup };
