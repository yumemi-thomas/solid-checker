import "fz-wany";

// A global written under a key nothing in the writer's file decides (`Object.assign(globalThis, loadPatches())`, `window[key]` with a key from a call) may be any global:
// every call of a global the library declares may run the writer's code, so each is a site (ADR 0973). Resolving the key where the file's constants decide it
// (`action-in-owned-scope-global-writes`) is what keeps the rows to the names that are written.
export function AnyGlobal() {
  btoa("x");
  atob("eA==");
  JSON.parse("1");
  Math.max(1, 2);
  new Map();
  return null;
}
