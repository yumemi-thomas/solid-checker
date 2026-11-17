export default {
  run(cb: () => unknown) {
    cb();
    return 1;
  },
};
