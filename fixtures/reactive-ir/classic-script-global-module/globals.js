// A module: its top-level declarations are its own, so the standard
// `setTimeout` and `Promise` stay the global's.
function setTimeout(f) {
  f();
  return 0;
}
let Promise = class {
  static resolve() {
    return { then(f) { f(); } };
  }
};
export { setTimeout as later, Promise as Eager };
