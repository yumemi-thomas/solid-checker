const make = () =>
  function (this: any) {
    this.Function("0")();
  };
setTimeout(make());
export {};
