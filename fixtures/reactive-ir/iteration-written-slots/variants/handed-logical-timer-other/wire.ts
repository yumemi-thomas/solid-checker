const maybe: any = (window as any).custom;
(maybe || setTimeout)(function (this: any) {
  this.Function("0")();
});
export {};
