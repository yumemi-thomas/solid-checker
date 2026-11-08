(window as any).Promise = class {
  constructor(f: (done: () => void) => void) {
    f(function (this: any) {
      this.Function("0")();
    });
  }
};
new Promise((done) => setTimeout(done, 0));
export {};
