const fake = function (this: any) {
  this.Function("0")();
};
new Promise(function f(done: any) {
  if (done !== fake) f(fake);
  setTimeout(done, 0);
});
export {};
