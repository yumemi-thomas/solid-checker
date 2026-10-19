export function keep(cb: () => void) {
  function run() {
    cb();
  }
  return run;
}
