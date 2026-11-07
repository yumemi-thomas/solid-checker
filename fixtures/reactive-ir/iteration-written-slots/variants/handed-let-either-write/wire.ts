let wake: any;
wake = () => {};
wake = function (this: any) {
  this.Function("0")();
};
setTimeout(wake, 0);
export {};
