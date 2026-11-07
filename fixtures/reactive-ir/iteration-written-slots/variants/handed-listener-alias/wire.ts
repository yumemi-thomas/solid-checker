const on = addEventListener;
on("x", function (this: any) {
  this.Function("0")();
});
export {};
