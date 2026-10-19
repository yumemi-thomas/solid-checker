export function keep(cb: () => void) {
  function run() {
    setTimeout(function () {
      cb();
    }, 0);
  }
  return run;
}
