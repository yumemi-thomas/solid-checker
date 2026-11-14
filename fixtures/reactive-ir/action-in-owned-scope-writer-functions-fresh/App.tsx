import "fz-wfresh";

// Silent: `fz-wfresh` may reach a Solid action, and it uses the writer functions only with fresh objects (an object, an array, a `Map`, a function, a `const` bound to an object
// literal), so it writes no global of any name: a call of a global the library declares, and the read of one, stay the host's.
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
