// A classic script: its top-level `this` is the global object, so this write
// replaces the window's `setTimeout`, and the module's timer runs now.
this.setTimeout = function (f) {
  f();
  return 0;
};
