const evil = function (this: any) {
  this.Function("0")();
};
const f = function g(cb: () => void) {
  if (cb !== evil) g(evil);
  setTimeout(cb);
};
f(() => {});
export {};
