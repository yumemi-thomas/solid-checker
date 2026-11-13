export const makeCalm = () => ({
  run(cb) {
    return [cb].length + Math.max(1, 2);
  },
});
