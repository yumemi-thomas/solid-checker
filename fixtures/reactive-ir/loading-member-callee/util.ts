// A default export that is an object literal holding the method.
export default {
  run(cb: () => unknown) {
    cb();
    return 1;
  },
};
