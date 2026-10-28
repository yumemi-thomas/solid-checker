// An imported helper that calls its callback in its own body.
export function runNow(cb: () => unknown) {
  cb();
}
