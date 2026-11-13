import "fz-wany";

// A global written under a name the syntax does not fix (`Object.assign(globalThis, source)`) may be any global: every call of a global the library declares is a site while
// a writer that may reach a Solid action is loaded.
export function AnyGlobal() {
  btoa("x"); // the one `source` holds
  atob("eA=="); // not written, but the name is not fixed
  return null;
}
