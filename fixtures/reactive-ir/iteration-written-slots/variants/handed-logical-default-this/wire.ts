function start(options: { run?: (f: () => void) => void }) {
  const run =
    options.run ??
    ((f: () => void) => {
      setTimeout(f, 0);
    });
  run(function (this: any) {
    this.Function("0")();
  });
}
start({});
export {};
