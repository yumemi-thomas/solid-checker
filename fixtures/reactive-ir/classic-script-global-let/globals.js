// A classic script: its top-level `let` is the global lexical `Promise` every
// module resolves through, and its `then` runs the callback now.
let Promise = class {
  static resolve() {
    return { then(f) { f(); } };
  }
};
