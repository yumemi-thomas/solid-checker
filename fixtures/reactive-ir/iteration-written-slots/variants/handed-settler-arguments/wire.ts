const evil = function (this: any) {
  this.Function("0")();
};
new Promise(function (done) {
  arguments[0] = evil;
  setTimeout(done, 0);
});
export {};
