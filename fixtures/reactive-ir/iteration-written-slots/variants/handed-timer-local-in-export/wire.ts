export function start(options: { run?: (f: () => void) => void }) {
  const run =
    options.run ??
    ((f: () => void) => {
      setTimeout(f, 0);
    });
  run(() => {});
}
start({});
