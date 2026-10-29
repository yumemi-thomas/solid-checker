export function runNow(cb: () => unknown) {
  cb();
  return 1;
}
