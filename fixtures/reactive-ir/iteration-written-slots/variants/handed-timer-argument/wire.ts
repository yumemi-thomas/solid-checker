const f = function (this: any) {
  this.Function("0")();
};
[f].forEach(setTimeout);
export {};
