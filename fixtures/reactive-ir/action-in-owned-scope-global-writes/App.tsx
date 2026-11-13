import "fz-wtimer";
import "fz-wfetch";
import "fz-wdefine";
import "fz-wassign";
import "fz-wclear";

// A global the library declares with its own type, which an installed package replaces where it loads: the call runs the package's code. The types are sound for the
// declaration; whether a package writes the global at run time is a fact of the packages' code.
export function WrittenTimer() {
  setTimeout(() => {}, 0); // `globalThis.setTimeout = …`
  return null;
}
export function WrittenThroughWindow() {
  void window.fetch("/x"); // `window.fetch = …`, called as a member of the global object
  return null;
}
export function WrittenByDefine() {
  queueMicrotask(() => {}); // `Object.defineProperty(globalThis, "queueMicrotask", …)`
  return null;
}
export function WrittenByAssign() {
  structuredClone({}); // `Object.assign(globalThis, { structuredClone })`
  return null;
}

// Silent: the writer is clear (its closure names no Solid package, it cannot call an action), or no installed package writes the global.
export function WrittenByClearPackage() {
  btoa("x");
  return null;
}
export function Unwritten() {
  atob("eA==");
  JSON.parse("1");
  Math.max(1, 2);
  return null;
}
