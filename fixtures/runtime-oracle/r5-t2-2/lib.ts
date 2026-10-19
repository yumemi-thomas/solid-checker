export function keepB(cb: () => void) {
  function run() {
    cb();
  }
  return run;
}
export function keepA(cb: () => void) {
  function run() {
    cb();
  }
  cb();
  return run;
}
