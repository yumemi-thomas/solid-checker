function make() {
  return function (this: any) {
    this.Function("0")();
  };
}
const flush = make();
addEventListener("load", flush);
export {};
