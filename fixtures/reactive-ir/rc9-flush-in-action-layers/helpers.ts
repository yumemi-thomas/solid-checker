// A helper in another module that calls its callback in its own body.
export function runNow(cb: () => unknown) {
  cb();
  return 1;
}
