function later(run: () => void) {
  setTimeout(run, 0);
}
later(function (this: any) {
  this.Function("0")();
});
export {};
