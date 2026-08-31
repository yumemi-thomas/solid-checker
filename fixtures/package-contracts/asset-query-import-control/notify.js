// The shipped file both imports below name. As a module its export is exact:
// the callback runs on the caller's stack. Through the `?raw` loader the same
// file is only source text, so nothing here may be attributed to that binding.
export function notify(callback) {
  callback();
}

export default notify;
