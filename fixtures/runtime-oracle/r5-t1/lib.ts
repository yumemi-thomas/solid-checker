export function keep(cb: () => void) {
  function run() {
    cb();
  }
  cb();
  return run;
}
