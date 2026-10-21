export function keep(cb) {
  return cb;
}

export function wrap(cb) {
  return cb;
}
wrap = function (cb) {
  return () => {};
};
