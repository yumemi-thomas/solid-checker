import "fz-wreflect";
import "fz-walias";
import "fz-wbound";

// A writer function used where its target is not seen may write any global: `fz-wreflect` calls `Reflect.apply(Object.assign, null, [globalThis, …])`, `fz-walias` hands
// `Object.assign` an object a parameter names, `fz-wbound` defines an accessor on one. Each is a package that may reach a Solid action and may have written ANY global, so a
// call of a global the library declares, or the read of one, is a site.
export function CallsFetch() {
  void fetch("/x");
  return null;
}
export function CallsTimer() {
  setTimeout(() => {}, 0);
  return null;
}
export function ReadsGlobal() {
  void window.innerWidth;
  return null;
}
